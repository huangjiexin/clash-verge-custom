use super::Group;
use crate::{
    config::{Config, IProfiles, profiles::PROFILE_WRITE_LOCK},
    core::{CoreManager, handle::Handle},
    process::AsyncHandler,
    utils::{dirs, help},
};
use anyhow::{Context as _, Result, bail};
use chrono::Utc;
use futures::{StreamExt as _, stream};
use parking_lot::Mutex;
use reqwest::{Client, Proxy, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_yaml_ng::{Mapping, Value};
use std::{
    collections::{HashMap, HashSet},
    hash::{DefaultHasher, Hasher as _},
    net::TcpListener,
    path::PathBuf,
    process::Stdio,
    sync::{
        LazyLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tokio::{fs, process::Command, time::sleep};

const MAX_BODY: usize = 1024 * 1024;
static SCAN_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static STARTED: AtomicBool = AtomicBool::new(false);
static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));
static FORCE: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Debug, Clone, Deserialize, Serialize, specta::Type)]
#[serde(default, deny_unknown_fields, rename_all = "kebab-case")]
pub struct Options {
    pub interval: u64,
    pub concurrency: usize,
    pub interface_name: String,
    pub checks: Vec<Check>,
    pub automatic: bool,
    pub sort: Sort,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    #[default]
    Delay,
    Name,
    Original,
}

#[derive(Debug, Clone, Deserialize, Serialize, specta::Type)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Check {
    pub url: String,
    pub expected_status: u16,
    #[serde(default)]
    pub validation: Validation,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, specta::Type)]
#[serde(rename_all = "kebab-case")]
pub enum Validation {
    #[default]
    Status,
    AuthJson,
    Html,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            interval: 300,
            concurrency: 16,
            interface_name: String::new(),
            checks: vec![],
            automatic: false,
            sort: Sort::Delay,
        }
    }
}

impl Options {
    fn validate(&self) -> Result<()> {
        if !(1..=32).contains(&self.concurrency) {
            bail!("concurrency must be between 1 and 32 nodes");
        }
        if !(60..=3600).contains(&self.interval) {
            bail!("interval must be between 60 and 3600 seconds");
        }
        if !self.checks.is_empty() && self.interface_name.trim().is_empty() {
            bail!("interface-name must name a physical outbound interface");
        }
        if self.automatic && self.checks.is_empty() {
            bail!("automatic selection requires a health-check URL");
        }
        for check in &self.checks {
            let url = reqwest::Url::parse(&check.url)?;
            if !["http", "https"].contains(&url.scheme())
                || url.host_str().is_none()
                || !url.username().is_empty()
                || url.password().is_some()
            {
                bail!("checks require an HTTP(S) URL without embedded credentials");
            }
            if !(100..=599).contains(&check.expected_status) {
                bail!("invalid expected HTTP status");
            }
        }
        Ok(())
    }
}

#[derive(Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NodeResult {
    pub name: String,
    pub passed: bool,
    pub delay: Option<u64>,
    pub reason: String,
    pub checked_at: String,
}

#[derive(Clone, Default, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub group_id: String,
    pub running: bool,
    pub completed: usize,
    pub total: usize,
    pub last_scan: Option<String>,
    pub error: Option<String>,
    pub nodes: Vec<NodeResult>,
    pub options: Options,
    pub logs: Vec<String>,
    pub active_nodes: usize,
    pub using_previous: bool,
}

impl Report {
    fn begin(&mut self, request: &Request) {
        self.running = true;
        self.completed = 0;
        self.total = request.candidates.len();
        self.error = None;
        self.options = request.options.clone();
    }

    fn record_node(&mut self, node: NodeResult) {
        self.completed += 1;
        if let Some(previous) = self.nodes.iter_mut().find(|previous| previous.name == node.name) {
            *previous = node;
        } else {
            self.nodes.push(node);
        }
    }
}

#[derive(Clone)]
struct Candidate {
    label: super::NodeLabel,
    name: String,
    proxy: Value,
    fingerprint: u64,
}

#[derive(Clone)]
pub(crate) struct Request {
    pub(super) id: u64,
    pub(super) group: Group,
    pub(super) options: Options,
    candidates: Vec<Candidate>,
    dns: Value,
    hosts: Value,
    logs: Vec<String>,
    error: Option<String>,
    serving: Snapshot,
}

#[derive(Clone, Default)]
struct Snapshot {
    request_id: u64,
    options: Options,
    candidates: Vec<Candidate>,
}

impl Snapshot {
    fn prepare(request: &Request, passed: Option<&HashMap<u64, u64>>) -> Self {
        let mut candidates: Vec<_> = request
            .candidates
            .iter()
            .filter(|candidate| passed.is_none_or(|passed| passed.contains_key(&candidate.fingerprint)))
            .cloned()
            .collect();
        match request.options.sort {
            Sort::Delay => candidates.sort_by_key(|candidate| {
                (
                    passed
                        .and_then(|passed| passed.get(&candidate.fingerprint))
                        .copied()
                        .unwrap_or(u64::MAX),
                    candidate.name.clone(),
                )
            }),
            Sort::Name => candidates.sort_by(|a, b| a.name.cmp(&b.name)),
            Sort::Original => {}
        }
        Self {
            request_id: request.id,
            options: request.options.clone(),
            candidates,
        }
    }

    fn same_members(&self, other: &Self) -> bool {
        self.request_id == other.request_id
            && self
                .candidates
                .iter()
                .map(|c| c.fingerprint)
                .eq(other.candidates.iter().map(|c| c.fingerprint))
    }
}

#[derive(Default)]
struct State {
    ready: HashMap<String, Snapshot>,
    reports: HashMap<String, Report>,
}

impl State {
    fn record(&mut self, request: &Request, passed: &HashMap<u64, u64>) -> bool {
        let snapshot = Snapshot::prepare(request, Some(passed));
        if snapshot.candidates.is_empty() {
            self.ready.remove(&request.group.id);
            return false;
        }
        let changed = !snapshot.same_members(&request.serving);
        self.ready.insert(request.group.id.clone(), snapshot);
        changed
    }

    fn serving(&self, request: &Request, previous: Option<&Request>) -> Snapshot {
        if request.error.is_none() && request.options.checks.is_empty() && !request.candidates.is_empty() {
            return Snapshot::prepare(request, None);
        }
        self.ready
            .get(&request.group.id)
            .filter(|snapshot| snapshot.request_id == request.id)
            .cloned()
            .or_else(|| previous.map(|previous| previous.serving.clone()))
            .unwrap_or_else(|| Snapshot {
                options: request.options.clone(),
                ..Snapshot::default()
            })
    }
}

fn fingerprint<T: std::hash::Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn is_candidate(proxy: &Value) -> bool {
    let Some(name) = proxy.get("name").and_then(Value::as_str) else {
        return false;
    };
    let kind = proxy.get("type").and_then(Value::as_str).unwrap_or_default();
    !["direct", "reject", "dns"].contains(&kind)
        && proxy.get("server").and_then(Value::as_str).is_some()
        && proxy.get("dialer-proxy").is_none()
        && !["剩余", "流量", "到期", "套餐", "重置", "过期", "官网", "公告", "订阅"]
            .iter()
            .any(|word| name.contains(word))
}

async fn collect_candidates(group: &Group, profiles: &IProfiles) -> Result<Vec<Candidate>> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    for uid in &group.sources {
        if !seen.insert(uid) {
            continue;
        }
        let item = profiles
            .items
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|item| item.uid.as_deref() == Some(uid.as_str()))
            .with_context(|| format!("Source subscription {uid} no longer exists"))?;
        if ![Some("remote"), Some("local")].contains(&item.itype.as_deref()) {
            bail!("Source must be an imported subscription");
        }
        let file = item.file.as_deref().context("source has no file")?;
        let source = help::read_mapping(&dirs::app_profiles_dir()?.join(file)).await?;
        if source
            .get("proxy-providers")
            .and_then(Value::as_mapping)
            .is_some_and(|p| !p.is_empty())
        {
            bail!(
                "{} uses proxy-providers; import an inline-node subscription first",
                item.name.as_deref().unwrap_or(uid)
            );
        }
        let label = item.name.as_deref().unwrap_or(uid);
        for proxy in source.get("proxies").and_then(Value::as_sequence).into_iter().flatten() {
            if !is_candidate(proxy) {
                continue;
            }
            let original = proxy.get("name").and_then(Value::as_str).unwrap_or_default();
            let name = format!("SG/{}/{label}/{uid}/{original}", group.id);
            let mut proxy = proxy.clone();
            proxy
                .as_mapping_mut()
                .context("invalid proxy")?
                .insert("name".into(), name.clone().into());
            candidates.push(Candidate {
                label: super::NodeLabel {
                    name: original.into(),
                    source: label.into(),
                },
                name,
                proxy,
                fingerprint: 0,
            });
        }
    }
    Ok(candidates)
}

pub(super) async fn prepare(group: &Group, profiles: &IProfiles, config: &Mapping) -> Result<Request> {
    let source = collect_candidates(group, profiles).await?;
    let (options, candidates, logs) = evaluate(group, source).await?;
    let dns = config.get("dns").cloned().unwrap_or(Value::Mapping(Mapping::new()));
    let hosts = config.get("hosts").cloned().unwrap_or(Value::Mapping(Mapping::new()));
    let id = fingerprint(&(
        group.script.clone(),
        group.name.clone(),
        candidates.iter().map(|c| c.fingerprint).collect::<Vec<_>>(),
        serde_yaml_ng::to_string(&dns)?,
        serde_yaml_ng::to_string(&hosts)?,
    ));
    let request = Request {
        id,
        group: group.clone(),
        options,
        candidates,
        dns,
        hosts,
        logs: vec![],
        error: None,
        serving: Snapshot::default(),
    };
    Ok(Request { logs, ..request })
}

async fn evaluate(group: &Group, source: Vec<Candidate>) -> Result<(Options, Vec<Candidate>, Vec<String>)> {
    let input = serde_yaml_ng::to_value(json!({
        "proxies": source.iter().map(|c| &c.proxy).collect::<Vec<_>>(),
        "screening": Options::default()
    }))?
    .as_mapping()
    .cloned()
    .context("invalid script input")?;
    let (output, _, logs) =
        crate::enhance::run_extension_script(group.script.clone().into(), input, group.name.clone().into()).await;
    if let Some((_, message)) = logs.iter().find(|(level, _)| level == "exception") {
        bail!("{}: {message}", group.name);
    }
    let options: Options = serde_yaml_ng::from_value(
        output
            .get("screening")
            .cloned()
            .context("script must return screening options")?,
    )?;
    options.validate()?;
    let originals: HashMap<_, _> = source.iter().map(|c| (c.name.as_str(), &c.label)).collect();
    let mut candidates = Vec::new();
    let mut names = HashSet::new();
    for proxy in output
        .get("proxies")
        .and_then(Value::as_sequence)
        .context("script must return a proxies array")?
    {
        let name = proxy
            .get("name")
            .and_then(Value::as_str)
            .context("node has no name")?
            .to_owned();
        if !originals.contains_key(name.as_str()) || !names.insert(name.clone()) {
            bail!("script must preserve unique imported node names");
        }
        if !is_candidate(proxy) {
            continue;
        }
        let mut proxy = proxy.clone();
        if !options.interface_name.is_empty() {
            proxy
                .as_mapping_mut()
                .context("invalid proxy")?
                .insert("interface-name".into(), options.interface_name.clone().into());
        }
        let fingerprint = fingerprint(&(
            group.id.clone(),
            serde_yaml_ng::to_string(&proxy)?,
            serde_json::to_string(&options.checks)?,
        ));
        candidates.push(Candidate {
            label: originals[name.as_str()].clone(),
            name,
            proxy,
            fingerprint,
        });
    }
    Ok((
        options,
        candidates,
        logs.iter().map(|(level, text)| format!("{level}: {text}")).collect(),
    ))
}

pub(super) fn failed_request(group: &Group, config: &Mapping, error: String) -> Request {
    Request {
        id: fingerprint(&(group.id.clone(), error.clone())),
        group: group.clone(),
        options: Options::default(),
        candidates: vec![],
        dns: config.get("dns").cloned().unwrap_or_default(),
        hosts: config.get("hosts").cloned().unwrap_or_default(),
        logs: vec![],
        error: Some(error),
        serving: Snapshot::default(),
    }
}

pub(super) fn append(config: &mut Mapping, request: &mut Request, previous: Option<&Request>) -> Result<()> {
    // The committed runtime owns the serving set; completed scans only stage its replacement.
    request.serving = STATE.lock().serving(request, previous);
    append_serving(config, request)
}

fn append_serving(config: &mut Mapping, request: &Request) -> Result<()> {
    let eligible = &request.serving.candidates;
    let options = request.serving_options();
    let mut nodes = config
        .get("proxies")
        .and_then(Value::as_sequence)
        .cloned()
        .unwrap_or_default();
    let known: HashSet<_> = nodes
        .iter()
        .filter_map(|n| n.get("name").and_then(Value::as_str).map(str::to_owned))
        .collect();
    for c in eligible {
        if known.contains(&c.name) {
            bail!("screening node name conflicts with an existing proxy");
        }
        nodes.push(c.proxy.clone());
    }
    config.insert("proxies".into(), nodes.into());
    let names: Vec<_> = eligible.iter().map(|c| c.name.clone()).collect();
    let automatic_name = request.automatic_name();
    let mut automatic = names.clone();
    if automatic.is_empty() {
        automatic.push("REJECT".into());
    }
    let mut choices = Vec::new();
    let mut generated = Vec::new();
    if options.automatic {
        let check = &options.checks[0];
        generated.push(json!({"name": automatic_name, "type": "url-test", "proxies": automatic,
            "url": check.url, "expected-status": check.expected_status, "interval": 60,
            "timeout": 10000, "lazy": false, "tolerance": 100}));
        choices.push(automatic_name);
    }
    choices.extend(names);
    if choices.is_empty() {
        choices.push("REJECT".into());
    }
    let mut parent = json!({"name": request.group.name, "type": "select", "proxies": choices});
    if let Some(check) = options.checks.first() {
        parent["url"] = json!(check.url);
        if !options.automatic {
            parent["expected-status"] = json!(check.expected_status);
            parent["interval"] = json!(60);
            parent["lazy"] = json!(false);
            parent["timeout"] = json!(10000);
        }
    }
    generated.push(parent);
    let groups = config
        .entry(Value::from("proxy-groups"))
        .or_insert_with(|| Value::Sequence(vec![]))
        .as_sequence_mut()
        .context("proxy-groups must be an array")?;
    for group in generated {
        let value = serde_yaml_ng::to_value(group)?;
        if groups.iter().any(|existing| existing.get("name") == value.get("name")) {
            bail!(
                "Subscription group name conflicts with existing proxy group: {}",
                request.group.name
            );
        }
        groups.push(value);
    }
    Ok(())
}

impl Request {
    pub(super) fn node_labels(&self) -> impl Iterator<Item = (String, super::NodeLabel)> + '_ {
        self.serving
            .candidates
            .iter()
            .map(|c| (c.name.clone(), c.label.clone()))
    }

    pub(super) fn serving_options(&self) -> &Options {
        &self.serving.options
    }

    pub(super) fn automatic_name(&self) -> String {
        super::compat::automatic_name(&self.group)
    }
}

async fn current_requests() -> Vec<Request> {
    Config::runtime().await.data_arc().subscription_groups.clone()
}

async fn is_current(request: &Request) -> bool {
    current_requests()
        .await
        .iter()
        .any(|r| r.group.id == request.group.id && r.id == request.id)
}

pub(super) fn reports(requests: &[Request]) -> Vec<Report> {
    let state = STATE.lock();
    requests
        .iter()
        .map(|request| {
            let mut report = state.reports.get(&request.group.id).cloned().unwrap_or_default();
            report.group_id = request.group.id.clone();
            report.options = request.options.clone();
            if request.error.is_some() {
                report.error = request.error.clone();
            }
            report.logs = request.logs.clone();
            report.total = request.candidates.len();
            report.active_nodes = request.serving.candidates.len();
            report.using_previous = report.active_nodes > 0
                && (report.running
                    || report.error.is_some()
                    || request.serving.request_id != request.id
                    || state
                        .ready
                        .get(&request.group.id)
                        .is_some_and(|ready| !ready.same_members(&request.serving)));
            report
        })
        .collect()
}
pub(super) fn rescan(id: String) {
    FORCE.lock().insert(id);
}

pub(crate) fn start() {
    if STARTED.swap(true, Ordering::SeqCst) {
        return;
    }
    AsyncHandler::spawn(|| async {
        let mut last = HashMap::<String, (u64, Instant)>::new();
        let mut pending_reload = false;
        loop {
            sleep(Duration::from_secs(2)).await;
            if Handle::global().is_exiting() {
                break;
            }
            let requests = current_requests().await;
            last.retain(|id, _| requests.iter().any(|r| &r.group.id == id));
            {
                let mut state = STATE.lock();
                state.ready.retain(|id, _| requests.iter().any(|r| &r.group.id == id));
                state.reports.retain(|id, _| requests.iter().any(|r| &r.group.id == id));
            }
            for request in requests {
                if !is_current(&request).await {
                    continue;
                }
                let forced = FORCE.lock().remove(&request.group.id);
                let due = forced
                    || last.get(&request.group.id).is_none_or(|(id, at)| {
                        *id != request.id || at.elapsed() >= Duration::from_secs(request.options.interval)
                    });
                if due {
                    let profiles = Config::profiles().await.data_arc();
                    let config = Config::runtime().await.data_arc().config.clone().unwrap_or_default();
                    match prepare(&request.group, &profiles, &config).await {
                        Ok(fresh) if fresh.id != request.id => {
                            pending_reload = true;
                            reload_if_needed(&mut pending_reload).await;
                            continue;
                        }
                        Err(error) => {
                            {
                                let mut state = STATE.lock();
                                let report = state.reports.entry(request.group.id.clone()).or_default();
                                report.error = Some(error.to_string());
                                state.ready.remove(&request.group.id);
                                drop(state);
                                last.insert(request.group.id.clone(), (request.id, Instant::now()));
                            }
                            reload_if_needed(&mut pending_reload).await;
                            continue;
                        }
                        _ => {}
                    }
                    if request.options.checks.is_empty() {
                        let mut state = STATE.lock();
                        let report = state.reports.entry(request.group.id.clone()).or_default();
                        if !request.candidates.is_empty() {
                            report.nodes = request
                                .candidates
                                .iter()
                                .map(|c| NodeResult {
                                    name: c.name.clone(),
                                    passed: true,
                                    delay: None,
                                    reason: "Selected by script; connectivity not tested".into(),
                                    checked_at: Utc::now().to_rfc3339(),
                                })
                                .collect();
                        }
                        report.last_scan = Some(Utc::now().to_rfc3339());
                        report.error = request
                            .candidates
                            .is_empty()
                            .then(|| "No nodes selected by the script".into());
                        drop(state);
                        last.insert(request.group.id.clone(), (request.id, Instant::now()));
                        continue;
                    }
                    {
                        let mut state = STATE.lock();
                        let report = state.reports.entry(request.group.id.clone()).or_default();
                        report.begin(&request);
                        drop(state);
                    }
                    let result = scan(&request).await;
                    last.insert(request.group.id.clone(), (request.id, Instant::now()));
                    let current = is_current(&request).await;
                    let mut state = STATE.lock();
                    let report = state.reports.entry(request.group.id.clone()).or_default();
                    report.running = false;
                    report.last_scan = Some(Utc::now().to_rfc3339());
                    if !current {
                        continue;
                    }
                    match result {
                        Ok(passed) => {
                            report
                                .nodes
                                .retain(|node| request.candidates.iter().any(|c| c.name == node.name));
                            if passed.is_empty() {
                                report.error = Some("No nodes passed the latest scan".into());
                            }
                            pending_reload |= state.record(&request, &passed);
                        }
                        Err(error) => {
                            report.error = Some(error.to_string());
                            state.ready.remove(&request.group.id);
                        }
                    }
                }
                reload_if_needed(&mut pending_reload).await;
            }
        }
    });
}

async fn reload_if_needed(pending: &mut bool) {
    if !*pending || Handle::global().is_exiting() {
        return;
    }
    let _write = PROFILE_WRITE_LOCK.lock().await;
    match CoreManager::global().update_config_checked().await {
        Ok(()) => {
            *pending = false;
            Handle::refresh_clash();
        }
        Err(error) => log::warn!("Could not apply subscription groups: {error:#}"),
    }
}

struct ProbeDirectory(PathBuf);
impl Drop for ProbeDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) async fn finish_on_exit() {
    let _scan = SCAN_LOCK.lock().await;
}

async fn collect_checks<F: std::future::Future>(
    checks: impl IntoIterator<Item = F>,
    concurrency: usize,
) -> Vec<F::Output> {
    stream::iter(checks).buffer_unordered(concurrency).collect().await
}

async fn scan(request: &Request) -> Result<HashMap<u64, u64>> {
    let _scan = SCAN_LOCK.lock().await;
    if Handle::global().is_exiting() {
        bail!("application is exiting");
    }
    if request.candidates.is_empty() {
        return Ok(HashMap::new());
    }
    let root = dirs::app_home_dir()?.join(format!("codex-probe-{}", nanoid::nanoid!()));
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        std::fs::DirBuilder::new().mode(0o700).create(&root)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir(&root)?;
    let directory = ProbeDirectory(root);
    let mut sockets = Vec::new();
    let mut listeners = Vec::new();
    let mut ports = Vec::new();
    for candidate in &request.candidates {
        let socket = TcpListener::bind("127.0.0.1:0")?;
        let port = socket.local_addr()?.port();
        ports.push(port);
        listeners.push(
            json!({"name": format!("probe-{port}"), "type": "http", "listen": "127.0.0.1",
            "port": port, "proxy": candidate.name, "udp": false}),
        );
        sockets.push(socket);
    }
    let mut dns = request.dns.clone();
    if let Some(dns) = dns.as_mapping_mut() {
        dns.insert("enable".into(), true.into());
        dns.remove("listen");
        dns.insert("respect-rules".into(), false.into());
    }
    let config = json!({
        "mixed-port": 0, "allow-lan": false, "mode": "rule", "ipv6": false, "log-level": "silent",
        "tun": {"enable": false}, "interface-name": request.options.interface_name,
        "dns": dns, "hosts": request.hosts, "profile": {"store-selected": false, "store-fake-ip": false},
        "proxies": request.candidates.iter().map(|c| &c.proxy).collect::<Vec<_>>(),
        "listeners": listeners, "rules": ["MATCH,REJECT"]
    });
    let path = directory.0.join("probe.yaml");
    fs::write(&path, serde_yaml_ng::to_string(&config)?).await?;
    let core = Config::verge().await.latest_arc().get_valid_clash_core();
    let binary = std::env::current_exe()?.with_file_name(format!("{core}{}", std::env::consts::EXE_SUFFIX));
    drop(sockets);
    let mut child = Command::new(binary)
        .args(["-d"])
        .arg(&directory.0)
        .arg("-f")
        .arg(&path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let probe = async {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if child.try_wait()?.is_some() {
                bail!("isolated probe core exited before becoming ready");
            }
            if tokio::net::TcpStream::connect((std::net::Ipv4Addr::LOCALHOST, ports[0]))
                .await
                .is_ok()
            {
                break;
            }
            if Instant::now() >= deadline {
                bail!("isolated probe core did not become ready");
            }
            sleep(Duration::from_millis(100)).await;
        }
        let results = collect_checks(
            request
                .candidates
                .clone()
                .into_iter()
                .zip(ports)
                .map(|(candidate, port)| async move {
                    let result = check_node(port, &request.options.checks).await;
                    let delay = result.as_ref().ok().copied();
                    let mut state = STATE.lock();
                    if let Some(report) = state.reports.get_mut(&request.group.id) {
                        report.record_node(NodeResult {
                            name: candidate.name.clone(),
                            passed: result.is_ok(),
                            delay,
                            reason: result
                                .err()
                                .map_or_else(|| "Passed all checks".into(), |e| e.to_string()),
                            checked_at: Utc::now().to_rfc3339(),
                        });
                    }
                    drop(state);
                    (candidate.fingerprint, delay)
                }),
            request.options.concurrency,
        )
        .await;
        Ok(results
            .into_iter()
            .filter_map(|(key, result)| result.map(|result| (key, result)))
            .collect())
    };
    let cancelled = async {
        loop {
            sleep(Duration::from_secs(1)).await;
            if Handle::global().is_exiting() || !is_current(request).await {
                break;
            }
        }
    };
    let outcome = tokio::select! {
        result = probe => result,
        () = cancelled => Err(anyhow::anyhow!("screening was cancelled")),
    };
    let _ = child.kill().await;
    let _ = child.wait().await;
    outcome
}

async fn body(client: &Client, url: &str) -> Result<(StatusCode, String)> {
    let mut response = client.get(url).send().await?;
    let status = response.status();
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if bytes.len() + chunk.len() > MAX_BODY {
            bail!("probe response exceeds size limit");
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((status, String::from_utf8(bytes)?))
}

fn authentication_response(status: StatusCode, body: &str) -> bool {
    if status != StatusCode::UNAUTHORIZED {
        return false;
    }
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return false;
    };
    let error = value.get("error").unwrap_or(&value);
    let message = error
        .get("message")
        .or_else(|| value.get("detail"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_lowercase();
    !body.contains("unsupported_country_region_territory")
        && [
            "api key",
            "authentication",
            "unauthorized",
            "missing bearer",
            "not authenticated",
            "access token",
        ]
        .iter()
        .any(|word| message.contains(word))
}

async fn check_node(port: u16, checks: &[Check]) -> Result<u64> {
    let client = Client::builder()
        .no_proxy()
        .proxy(Proxy::all(format!("http://127.0.0.1:{port}"))?)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .user_agent("Mozilla/5.0")
        .build()?;
    let mut delay = 0;
    for (index, check) in checks.iter().enumerate() {
        let started = Instant::now();
        let (status, content) = body(&client, &check.url).await.map_err(|error| {
            let reason = if error
                .downcast_ref::<reqwest::Error>()
                .is_some_and(reqwest::Error::is_timeout)
            {
                "timeout"
            } else {
                "connection or response error"
            };
            anyhow::anyhow!("Check {}: {reason}", index + 1)
        })?;
        if status.as_u16() != check.expected_status {
            bail!(
                "Check {}: HTTP {}, expected {}",
                index + 1,
                status.as_u16(),
                check.expected_status
            );
        }
        let valid = match check.validation {
            Validation::Status => true,
            Validation::AuthJson => authentication_response(status, &content),
            Validation::Html => {
                status == StatusCode::OK
                    && content.to_lowercase().contains("<html")
                    && !content.contains("cf-chl")
                    && !content.contains("Just a moment")
            }
        };
        if !valid {
            bail!(
                "Check {}: response body did not satisfy {:?}",
                index + 1,
                check.validation
            );
        }
        if index == 0 {
            delay = started.elapsed().as_millis().max(1) as u64;
        }
    }
    Ok(delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_false_positive_health_responses() {
        assert!(authentication_response(
            StatusCode::UNAUTHORIZED,
            r#"{"error":{"message":"You didn't provide an API key."}}"#
        ));
        assert!(authentication_response(
            StatusCode::UNAUTHORIZED,
            r#"{"detail":"Unauthorized"}"#
        ));
        assert!(!authentication_response(
            StatusCode::METHOD_NOT_ALLOWED,
            r#"{"detail":"Method Not Allowed"}"#
        ));
        assert!(!authentication_response(
            StatusCode::FORBIDDEN,
            r#"{"error":{"code":"unsupported_country_region_territory","message":"Unauthorized"}}"#
        ));
        assert!(!authentication_response(
            StatusCode::UNAUTHORIZED,
            "<html>Just a moment</html>"
        ));
    }
    #[tokio::test]
    async fn scans_use_the_configured_concurrency_and_keep_every_result() {
        use std::sync::{Arc, atomic::AtomicUsize};
        for limit in [1, 4, 16, 32] {
            let active = Arc::new(AtomicUsize::new(0));
            let peak = Arc::new(AtomicUsize::new(0));
            let jobs = (0..40).map(|id| {
                let active = active.clone();
                let peak = peak.clone();
                async move {
                    let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(count, Ordering::SeqCst);
                    tokio::task::yield_now().await;
                    active.fetch_sub(1, Ordering::SeqCst);
                    id
                }
            });
            let mut results = collect_checks(jobs, limit).await;
            results.sort_unstable();
            assert_eq!(results, (0..40).collect::<Vec<_>>());
            assert_eq!(peak.load(Ordering::SeqCst), limit);
            assert_eq!(active.load(Ordering::SeqCst), 0);
        }
        let options: Options = serde_json::from_str("{}").unwrap();
        assert_eq!(options.concurrency, 16);
        for concurrency in [0, 33] {
            assert!(
                Options {
                    concurrency,
                    ..Options::default()
                }
                .validate()
                .is_err()
            );
        }
    }

    fn checked_request(id: u64, group_id: &str, node_ids: &[u64]) -> Request {
        let group = Group {
            id: group_id.into(),
            name: format!("Group {group_id}"),
            enabled: true,
            sources: vec![],
            script: String::new(),
        };
        let mut request = failed_request(&group, &Mapping::new(), String::new());
        request.id = id;
        request.error = None;
        request.options = Options {
            interval: 3600,
            automatic: true,
            checks: vec![Check {
                url: format!("https://check-{id}.example/health"),
                expected_status: 204,
                validation: Validation::Status,
            }],
            ..Options::default()
        };
        request.candidates = node_ids
            .iter()
            .map(|id| Candidate {
                label: super::super::NodeLabel {
                    name: format!("Node {id}"),
                    source: "Source".into(),
                },
                name: format!("node-{id}"),
                proxy: serde_yaml_ng::to_value(json!({
                    "name": format!("node-{id}"), "type": "ss", "server": format!("node-{id}.example"),
                    "port": 443, "cipher": "aes-128-gcm", "password": "test"
                }))
                .unwrap(),
                fingerprint: *id,
            })
            .collect();
        request
    }

    #[test]
    fn refresh_keeps_the_committed_set_until_a_complete_replacement_is_ready() {
        let mut state = State::default();
        let mut committed = checked_request(1, "one", &[1, 2]);
        assert!(state.record(&committed, &HashMap::from([(1, 100), (2, 200)])));
        committed.serving = state.serving(&committed, None);
        let mut before = Mapping::new();
        append_serving(&mut before, &committed).unwrap();

        let mut draft = checked_request(2, "one", &[3, 4]);
        draft.serving = state.serving(&draft, Some(&committed));
        let mut pending = Mapping::new();
        append_serving(&mut pending, &draft).unwrap();
        assert_eq!(pending, before);
        assert_eq!(
            draft.node_labels().map(|(name, _)| name).collect::<Vec<_>>(),
            vec!["node-1", "node-2"]
        );
        assert_eq!(draft.serving_options().checks[0].url, committed.options.checks[0].url);

        assert!(state.record(&draft, &HashMap::from([(4, 50), (3, 100)])));
        let mut still_serving = Mapping::new();
        append_serving(&mut still_serving, &committed).unwrap();
        assert_eq!(still_serving, before);

        draft.serving = state.serving(&draft, Some(&committed));
        let mut after = Mapping::new();
        append_serving(&mut after, &draft).unwrap();
        assert_eq!(after.get("proxies").unwrap().as_sequence().unwrap().len(), 2);
        assert_eq!(
            draft.node_labels().map(|(name, _)| name).collect::<Vec<_>>(),
            vec!["node-4", "node-3"]
        );
        assert_eq!(draft.serving_options().checks[0].url, draft.options.checks[0].url);

        // Rejecting a draft leaves the runtime's previous proxy definitions intact.
        drop(draft);
        let mut rejected = Mapping::new();
        append_serving(&mut rejected, &committed).unwrap();
        assert_eq!(rejected, before);

        let failed_refresh = checked_request(2, "one", &[3, 4]);
        assert!(!state.record(&failed_refresh, &HashMap::new()));
        assert!(
            state
                .serving(&failed_refresh, Some(&committed))
                .same_members(&committed.serving)
        );
    }

    #[test]
    fn empty_and_broken_updates_keep_each_groups_previous_snapshot() {
        let mut state = State::default();
        let mut committed = checked_request(1, "one", &[1, 2]);
        state.record(&committed, &HashMap::from([(1, 100)]));
        committed.serving = state.serving(&committed, None);
        for request in [
            checked_request(1, "one", &[1, 2]),
            checked_request(2, "one", &[3]),
            checked_request(3, "one", &[]),
            failed_request(&committed.group, &Mapping::new(), "Source unavailable".into()),
        ] {
            assert!(!state.record(&request, &HashMap::new()));
            let snapshot = state.serving(&request, Some(&committed));
            assert!(snapshot.same_members(&committed.serving));
            assert_eq!(snapshot.candidates[0].proxy, committed.serving.candidates[0].proxy);
        }
        let unrelated = checked_request(1, "two", &[1]);
        assert!(state.serving(&unrelated, None).candidates.is_empty());
        assert!(
            state
                .serving(&committed, Some(&committed))
                .same_members(&committed.serving)
        );
    }

    #[test]
    fn scan_progress_preserves_previous_results_and_replaces_matching_rows() {
        let request = checked_request(1, "one", &[1, 2]);
        let mut report = Report {
            last_scan: Some("previous completion".into()),
            nodes: vec![NodeResult {
                name: "node-1".into(),
                passed: true,
                ..NodeResult::default()
            }],
            ..Report::default()
        };
        report.begin(&request);
        assert!(report.running);
        assert_eq!(report.completed, 0);
        assert_eq!(report.nodes.len(), 1);
        assert_eq!(report.last_scan.as_deref(), Some("previous completion"));
        report.record_node(NodeResult {
            name: "node-1".into(),
            passed: false,
            ..NodeResult::default()
        });
        report.record_node(NodeResult {
            name: "node-2".into(),
            passed: true,
            ..NodeResult::default()
        });
        assert_eq!(report.completed, 2);
        assert_eq!(report.nodes.len(), 2);
        assert!(!report.nodes[0].passed);
        report.begin(&request);
        assert_eq!(report.nodes.len(), 2);
    }

    #[tokio::test]
    async fn scripts_keep_group_logic_separate_and_fail_closed() {
        let source = || {
            ["US", "JP"]
                .into_iter()
                .map(|name| Candidate {
                    label: super::super::NodeLabel {
                        name: name.into(),
                        source: "Source/with slash".into(),
                    },
                    name: name.into(),
                    proxy: serde_yaml_ng::to_value(
                        json!({"name":name,"type":"ss","server":"example.invalid","port":443}),
                    )
                    .unwrap(),
                    fingerprint: 0,
                })
                .collect::<Vec<_>>()
        };
        let mut group = Group {
            id: "one".into(),
            name: "One".into(),
            enabled: true,
            sources: vec![],
            script: "function main(c) { c.proxies = c.proxies.filter(p => p.name === 'US'); return c; }".into(),
        };
        let (_, first, _) = evaluate(&group, source()).await.unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].name, "US");
        assert_eq!(first[0].label.name, "US");
        assert_eq!(first[0].label.source, "Source/with slash");
        group.id = "two".into();
        group.script = "function main(c) { c.proxies = c.proxies.filter(p => p.name === 'JP'); return c; }".into();
        let (_, second, _) = evaluate(&group, source()).await.unwrap();
        assert_eq!(second[0].name, "JP");
        assert_ne!(first[0].fingerprint, second[0].fingerprint);
        group.script = "function main(c) { throw new Error('bad filter'); }".into();
        assert!(evaluate(&group, source()).await.is_err());
    }

    #[test]
    fn multiple_groups_compose_without_changing_rules() {
        let group = |id: &str, name: &str| Group {
            id: id.into(),
            name: name.into(),
            enabled: true,
            sources: vec![],
            script: String::new(),
        };
        let rules = serde_yaml_ng::to_value(vec!["MATCH,DIRECT"]).unwrap();
        let mut config = Mapping::new();
        config.insert("rules".into(), rules.clone());
        for (id, name) in [("one", "Codex"), ("two", "Claude"), ("three", "Company")] {
            let mut request = failed_request(&group(id, name), &config, "Missing source".into());
            append(&mut config, &mut request, None).unwrap();
        }
        assert_eq!(config.get("rules"), Some(&rules));
        let groups = config.get("proxy-groups").unwrap().as_sequence().unwrap();
        assert_eq!(groups.len(), 3);
        assert!(
            groups
                .iter()
                .all(|g| g.get("proxies").unwrap().as_sequence().unwrap() == &vec![Value::from("REJECT")])
        );
    }

    #[test]
    fn skips_metadata_and_dependent_outbounds() {
        let p = |name: &str| {
            serde_yaml_ng::to_value(json!({
                "name":name,"type":"vmess","server":"example.invalid","port":443
            }))
            .expect("valid fixture")
        };
        assert!(is_candidate(&p("Japan GPT")));
        assert!(!is_candidate(&p("剩余流量：100 GB")));
        let mut dependent = p("Japan GPT");
        dependent
            .as_mapping_mut()
            .expect("mapping")
            .insert("dialer-proxy".into(), "upstream".into());
        assert!(!is_candidate(&dependent));
        let mut changed = p("Japan GPT");
        changed
            .as_mapping_mut()
            .expect("mapping")
            .insert("uuid".into(), "new-credential".into());
        assert_ne!(
            fingerprint(&serde_yaml_ng::to_string(&p("Japan GPT")).expect("yaml")),
            fingerprint(&serde_yaml_ng::to_string(&changed).expect("yaml"))
        );
    }
}
