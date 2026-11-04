//! Metadata-only service with coalesced immutable revisions and immediate cancellation.
//! Directory handles anchor traversal: no path-based recursion and no symlink following.
use crate::data::{Category, CloudState, Coverage, Dataset, Node, NodeKind};
use std::{
    ffi::{CStr, CString, OsStr},
    io,
    os::unix::ffi::OsStrExt,
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

#[cfg(test)]
type MetadataHook = Arc<dyn Fn(&Path) + Send + Sync>;

pub struct Request {
    pub root: PathBuf,
    pub run_id: u64,
    #[cfg(any(test, debug_assertions))]
    pub metadata_delay: Duration,
    #[cfg(test)]
    pub metadata_pause: Option<(Arc<AtomicBool>, Arc<AtomicBool>)>,
    // Narrow metadata adapter, exclusively for deterministic boundary tests under
    // the same service. It never implements a second traversal/file-system API.
    #[cfg(test)]
    pub metadata_override: Option<(PathBuf, libc::dev_t)>,
    #[cfg(test)]
    pub before_metadata: Option<MetadataHook>,
    #[cfg(test)]
    pub dataless_paths: Vec<PathBuf>,
}
impl Request {
    pub fn new(root: PathBuf, run_id: u64) -> Self {
        Self {
            root,
            run_id,
            #[cfg(test)]
            metadata_override: None,
            #[cfg(test)]
            before_metadata: None,
            #[cfg(test)]
            dataless_paths: vec![],
            #[cfg(test)]
            metadata_pause: None,
            #[cfg(any(test, debug_assertions))]
            metadata_delay: Duration::ZERO,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Running,
    Complete,
    Limited,
    Unavailable,
    Cancelled,
    Failed,
}
#[derive(Clone, Debug)]
pub struct Problem {
    pub path: PathBuf,
    pub reason: String,
    io_kind: Option<io::ErrorKind>,
    pub permission_denied: bool,
    pub volume_boundary: bool,
}
impl Problem {
    fn io(path: PathBuf, error: io::Error) -> Self {
        Self {
            path,
            permission_denied: matches!(error.raw_os_error(), Some(libc::EACCES | libc::EPERM)),
            reason: error.to_string(),
            io_kind: Some(error.kind()),
            volume_boundary: false,
        }
    }
    fn note(path: PathBuf, reason: &str, volume_boundary: bool) -> Self {
        Self {
            path,
            reason: reason.into(),
            io_kind: None,
            permission_denied: false,
            volume_boundary,
        }
    }
    pub fn description(&self, cs: bool) -> &str {
        if let Some(kind) = self.io_kind {
            return match (kind, cs) {
                (io::ErrorKind::NotFound, true) => "Položka již neexistuje nebo není dostupná.",
                (io::ErrorKind::NotFound, false) => "Item no longer exists or is unavailable.",
                (io::ErrorKind::PermissionDenied, true) => "Ke čtení metadat chybí oprávnění.",
                (io::ErrorKind::PermissionDenied, false) => {
                    "Permission to read metadata was denied."
                }
                (io::ErrorKind::InvalidInput, true) => "Umístění nelze bezpečně skenovat.",
                (io::ErrorKind::InvalidInput, false) => "This location cannot be scanned safely.",
                (_, true) => "Metadata položky nelze načíst.",
                (_, false) => "Item metadata could not be read.",
            };
        }
        if let Some((en, localized)) = self.reason.split_once(" / ") {
            if cs { localized } else { en }
        } else {
            &self.reason
        }
    }
    pub fn system_detail(&self) -> Option<&str> {
        self.io_kind.map(|_| self.reason.as_str())
    }
    pub fn advice(&self, cs: bool) -> Option<&'static str> {
        self.permission_denied.then_some(if cs {
            "Ověřte oprávnění složky a v Nastavení systému → Soukromí a zabezpečení → Soubory a složky / Úplný přístup k disku povolte přístup aplikaci. Poté sken zopakujte."
        } else {
            "Check folder permissions and allow the app in System Settings → Privacy & Security → Files and Folders / Full Disk Access, then scan again."
        })
    }
}
#[derive(Clone)]
pub struct Event {
    pub run_id: u64,
    pub revision: u64,
    pub state: State,
    pub data: Option<Arc<Dataset>>,
    pub problems: Vec<Problem>,
}
#[derive(Default)]
struct Mailbox {
    pending: Option<Event>,
    latest: Option<Event>,
    terminal: bool,
}
impl Mailbox {
    fn publish(&mut self, event: Event) {
        if self.terminal {
            return;
        }
        self.terminal = event.state != State::Running;
        self.latest = Some(event.clone());
        self.pending = Some(event);
    }
}
pub struct Scan {
    mailbox: Arc<Mutex<Mailbox>>,
    cancel: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    started: Instant,
    run_id: u64,
}
impl Scan {
    pub fn start(request: Request) -> Self {
        let mailbox = Arc::new(Mutex::new(Mailbox::default()));
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let output = mailbox.clone();
        let run_id = request.run_id;
        let worker = std::thread::spawn(move || {
            let event = collect(request, &stop, &output);
            output.lock().unwrap().publish(event);
        });
        Self {
            mailbox,
            cancel,
            worker: Some(worker),
            started: Instant::now(),
            run_id,
        }
    }
    pub fn poll(&self) -> Result<Event, mpsc::TryRecvError> {
        let mut mailbox = self.mailbox.lock().unwrap();
        mailbox.pending.take().ok_or(if mailbox.terminal {
            mpsc::TryRecvError::Disconnected
        } else {
            mpsc::TryRecvError::Empty
        })
    }
    pub fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
    pub fn worker_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(|w| w.is_finished())
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        let mut mailbox = self.mailbox.lock().unwrap();
        let mut event = mailbox.latest.clone().unwrap_or(Event {
            run_id: self.run_id,
            revision: 0,
            state: State::Running,
            data: None,
            problems: vec![],
        });
        event.revision += 1;
        event.state = State::Cancelled;
        mailbox.publish(event);
    }
}
impl Drop for Scan {
    fn drop(&mut self) {
        self.cancel();
        if self.worker_finished()
            && let Some(worker) = self.worker.take()
        {
            let _ = worker.join();
        }
    }
}

fn snapshot(nodes: &[Node], run_id: u64) -> Arc<Dataset> {
    let mut nodes = nodes.to_vec();
    nodes[0].coverage = Coverage::Partial;
    aggregate(&mut nodes);
    Arc::new(Dataset::scanned(nodes, run_id))
}
fn aggregate(nodes: &mut [Node]) {
    // Choose the smallest native path among discovered aliases, independent of
    // directory enumeration order. Recompute per published revision so a newly
    // discovered earlier path can take ownership without changing node IDs.
    let mut owners = std::collections::HashMap::new();
    for (id, n) in nodes.iter().enumerate() {
        if n.coverage == Coverage::Complete
            && let Some(identity) = n.file_identity
        {
            let owner = owners.entry(identity).or_insert(id);
            if n.native_path < nodes[*owner].native_path {
                *owner = id;
            }
        }
    }
    for (id, n) in nodes.iter_mut().enumerate() {
        if let Some(owner) = n.file_identity.and_then(|key| owners.get(&key)).copied()
            && n.coverage == Coverage::Complete
        {
            n.hard_link_owner = (owner != id).then_some(owner);
            (n.logical, n.allocated) = if owner == id {
                n.reported_size.unwrap()
            } else {
                (0, 0)
            };
        }
    }
    for id in (1..nodes.len()).rev() {
        let p = nodes[id].parent.unwrap();
        nodes[p].logical = nodes[p].logical.saturating_add(nodes[id].logical);
        nodes[p].allocated = nodes[p].allocated.saturating_add(nodes[id].allocated);
        if nodes[id].coverage != Coverage::Complete {
            nodes[p].coverage = Coverage::Partial;
        }
    }
}

// Owned DIR closes the underlying descriptor even on error. A stack keeps depth
// off the call stack; at most one open directory per nesting level is retained.
struct Directory(*mut libc::DIR);
impl Drop for Directory {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}
impl Directory {
    fn fd(&self) -> i32 {
        unsafe { libc::dirfd(self.0) }
    }
    fn open(
        parent: i32,
        name: &CStr,
        device: Option<libc::dev_t>,
    ) -> io::Result<(Self, libc::stat)> {
        // O_DIRECTORY prevents opening streams/devices; O_NOFOLLOW also closes the
        // stat/open race. fstat checks the opened object and mount boundary again.
        let fd = unsafe {
            libc::openat(
                parent,
                name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_DIRECTORY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut st = std::mem::MaybeUninit::uninit();
        if unsafe { libc::fstat(fd, st.as_mut_ptr()) } != 0 {
            let e = io::Error::last_os_error();
            unsafe {
                libc::close(fd);
            }
            return Err(e);
        }
        let st = unsafe { st.assume_init() };
        if device.is_some_and(|d| d != st.st_dev) || dataless(&st) {
            unsafe {
                libc::close(fd);
            }
            return Err(io::Error::other(
                "Volume boundary or cloud metadata changed",
            ));
        }
        let dir = unsafe { libc::fdopendir(fd) };
        if dir.is_null() {
            let e = io::Error::last_os_error();
            unsafe {
                libc::close(fd);
            }
            return Err(e);
        }
        Ok((Self(dir), st))
    }
    fn metadata(&self) -> io::Result<libc::stat> {
        let mut st = std::mem::MaybeUninit::uninit();
        if unsafe { libc::fstat(self.fd(), st.as_mut_ptr()) } == 0 {
            Ok(unsafe { st.assume_init() })
        } else {
            Err(io::Error::last_os_error())
        }
    }
    fn next(&self) -> io::Result<Option<CString>> {
        // readdir uses errno to distinguish end-of-directory from failure.
        unsafe {
            #[cfg(target_os = "macos")]
            let errno = libc::__error();
            #[cfg(target_os = "linux")]
            let errno = libc::__errno_location();
            *errno = 0;
            let entry = libc::readdir(self.0);
            if entry.is_null() {
                return if *errno == 0 {
                    Ok(None)
                } else {
                    Err(io::Error::last_os_error())
                };
            }
            Ok(Some(CStr::from_ptr((*entry).d_name.as_ptr()).to_owned()))
        }
    }
}
fn stat_at(fd: i32, name: &CStr) -> io::Result<libc::stat> {
    let mut st = std::mem::MaybeUninit::uninit();
    if unsafe {
        libc::fstatat(
            fd,
            name.as_ptr(),
            st.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { st.assume_init() })
    }
}
fn metadata_at(request: &Request, fd: i32, name: &CStr, path: &Path) -> io::Result<libc::stat> {
    let result = stat_at(fd, name);
    #[cfg(test)]
    let result = result.map(|mut st| {
        if let Some((target, device)) = &request.metadata_override
            && target == path
        {
            st.st_dev = *device;
        }
        #[cfg(target_os = "macos")]
        if request.dataless_paths.iter().any(|p| p == path) {
            st.st_flags |= 0x40000000;
        }
        st
    });
    #[cfg(not(test))]
    let _ = (request, path);
    result
}
fn dataless(st: &libc::stat) -> bool {
    #[cfg(target_os = "macos")]
    {
        st.st_flags & 0x40000000 != 0
    } // SF_DATALESS, sys/stat.h
    #[cfg(not(target_os = "macos"))]
    {
        let _ = st;
        false
    }
}
fn cloud_path(path: &Path) -> bool {
    let parts: Vec<_> = path.components().collect();
    parts.windows(2).any(|p| {
        p[0].as_os_str().as_bytes().eq_ignore_ascii_case(b"Library")
            && (p[1]
                .as_os_str()
                .as_bytes()
                .eq_ignore_ascii_case(b"CloudStorage")
                || p[1]
                    .as_os_str()
                    .as_bytes()
                    .eq_ignore_ascii_case(b"Mobile Documents"))
    })
}
fn legacy_placeholder(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.as_bytes().to_ascii_lowercase().ends_with(b".icloud"))
}
fn category(path: &Path) -> Category {
    match path
        .extension()
        .and_then(OsStr::to_str)
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "rs" | "swift" | "kt" | "js" | "ts" | "toml" | "html" | "css" | "py" => Category::Projects,
        "mov" | "mp4" | "png" | "jpg" | "jpeg" | "wav" | "mp3" => Category::Media,
        "app" | "dmg" | "pkg" => Category::Apps,
        "pdf" | "txt" | "md" | "docx" | "xlsx" => Category::Documents,
        _ => Category::Other,
    }
}
fn node(path: PathBuf, parent: Option<usize>, st: Option<&libc::stat>) -> Node {
    let kind = st.map_or(NodeKind::Unknown, |s| match s.st_mode & libc::S_IFMT {
        libc::S_IFDIR => NodeKind::Directory,
        libc::S_IFREG => NodeKind::File,
        libc::S_IFLNK => NodeKind::Symlink,
        _ => NodeKind::Special,
    });
    let cloud = if st.is_some_and(dataless) {
        CloudState::Dataless
    } else if kind == NodeKind::File && legacy_placeholder(&path) {
        CloudState::LegacyPlaceholder
    } else if cloud_path(&path) {
        CloudState::Cached
    } else {
        CloudState::Local
    };
    let sized = matches!(kind, NodeKind::File | NodeKind::Symlink)
        && cloud != CloudState::LegacyPlaceholder;
    let reported_size = sized.then(|| {
        let st = st.unwrap();
        (
            st.st_size.max(0) as u64,
            (st.st_blocks.max(0) as u64).saturating_mul(512),
        )
    });
    let (logical, allocated) = reported_size.unwrap_or((0, 0));
    Node {
        name: path
            .file_name()
            .unwrap_or(path.as_os_str())
            .to_string_lossy()
            .into_owned(),
        category: category(&path),
        native_path: Some(path),
        kind,
        is_package: false,
        cloud,
        reported_size,
        file_identity: (kind == NodeKind::File && sized).then(|| {
            let st = st.unwrap();
            (st.st_dev, st.st_ino)
        }),
        hard_link_owner: None,
        parent,
        children: vec![],
        logical,
        allocated,
        coverage: if st.is_some() {
            Coverage::Complete
        } else {
            Coverage::Unknown
        },
    }
}
fn root_directory(root: &Path, request: &Request) -> io::Result<(Option<Directory>, libc::stat)> {
    // Resolve ancestors without following links. The worker I/O policy prevents
    // materialization, including a race with eviction between stat and open.
    let (mut dir, mut st) = Directory::open(libc::AT_FDCWD, c"/", None)?;
    let mut path = PathBuf::from("/");
    for component in root.components() {
        match component {
            Component::RootDir => continue,
            Component::Normal(name) => {
                path.push(name);
                let name = CString::new(name.as_bytes())?;
                let meta = metadata_at(request, dir.fd(), &name, &path)?;
                if dataless(&meta) {
                    if path == root && meta.st_mode & libc::S_IFMT == libc::S_IFDIR {
                        return Ok((None, meta));
                    }
                    return Err(io::Error::other(
                        "Cloud ancestor unavailable without materialization / cloudový předek nedostupný bez stažení",
                    ));
                }
                (dir, st) = Directory::open(dir.fd(), &name, None)?;
            }
            _ => {
                return Err(io::Error::other(
                    "Choose an absolute native path without parent components",
                ));
            }
        }
    }
    Ok((Some(dir), st))
}
fn collect(request: Request, stop: &AtomicBool, output: &Mutex<Mailbox>) -> Event {
    let mut event = Event {
        run_id: request.run_id,
        revision: 1,
        state: State::Failed,
        data: None,
        problems: vec![],
    };
    if let Err(error) = crate::platform::disable_materialization() {
        event.problems.push(Problem::io(request.root, error));
        return event;
    }
    let root = if request.root.is_absolute() {
        request.root.clone()
    } else {
        match std::env::current_dir() {
            Ok(p) => p.join(&request.root),
            Err(e) => {
                event.problems.push(Problem::io(request.root, e));
                return event;
            }
        }
    };
    let (dir, st) = match root_directory(&root, &request) {
        Ok(d) => d,
        Err(e) => {
            event.problems.push(Problem::io(root, e));
            return event;
        }
    };
    let device = st.st_dev;
    let mut nodes = vec![node(root.clone(), None, Some(&st))];
    let Some(dir) = dir else {
        nodes[0].coverage = Coverage::Unknown;
        event.state = State::Limited;
        event.problems.push(Problem::note(root,
            "Cloud folder metadata only: contents unavailable without materialization / cloudová složka: obsah nedostupný bez stažení", false));
        event.data = Some(Arc::new(Dataset::scanned(nodes, request.run_id)));
        return event;
    };
    match crate::platform::is_package(&root) {
        Ok(value) => nodes[0].is_package = value,
        Err(error) => event.problems.push(Problem::io(root.clone(), error)),
    }
    nodes[0].coverage = Coverage::Partial;
    let mut stack = vec![(0, dir, st)];
    let initial = snapshot(&nodes, request.run_id);
    output.lock().unwrap().publish(Event {
        run_id: request.run_id,
        revision: 1,
        state: State::Running,
        data: Some(initial),
        problems: vec![],
    });
    let mut published = Instant::now();
    let mut published_count = nodes.len();
    while let Some((parent, directory, before)) = stack.last() {
        if stop.load(Ordering::Relaxed) {
            event.state = State::Cancelled;
            break;
        }
        let parent = *parent;
        let name = match directory.next() {
            Ok(Some(n)) => n,
            other => {
                let path = nodes[parent].native_path.clone().unwrap();
                let problem = match other {
                    Err(e) => Some(Problem::io(path, e)),
                    Ok(_) => match directory.metadata() {
                        Err(e) => Some(Problem::io(path, e)),
                        Ok(after)
                            if before.st_mtime != after.st_mtime
                                || before.st_mtime_nsec != after.st_mtime_nsec
                                || before.st_ctime != after.st_ctime
                                || before.st_ctime_nsec != after.st_ctime_nsec =>
                        {
                            Some(Problem::note(
                                path,
                                "Directory changed during scan / složka se během skenu změnila",
                                false,
                            ))
                        }
                        Ok(_) => None,
                    },
                };
                nodes[parent].coverage = if let Some(problem) = problem {
                    event.problems.push(problem);
                    Coverage::Partial
                } else {
                    Coverage::Complete
                };
                stack.pop();
                continue;
            }
        };
        if name.as_bytes() == b"." || name.as_bytes() == b".." {
            continue;
        }
        let path = nodes[parent]
            .native_path
            .as_ref()
            .unwrap()
            .join(OsStr::from_bytes(name.as_bytes()));
        #[cfg(test)]
        if let Some((entered, release)) = &request.metadata_pause {
            entered.store(true, Ordering::SeqCst);
            while !release.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if stop.load(Ordering::Relaxed) {
            event.state = State::Cancelled;
            break;
        }
        #[cfg(any(test, debug_assertions))]
        std::thread::sleep(request.metadata_delay);
        #[cfg(test)]
        if let Some(hook) = &request.before_metadata {
            hook(&path);
        }
        let meta = metadata_at(&request, directory.fd(), &name, &path);
        let id = nodes.len();
        let mut n = node(path.clone(), Some(parent), meta.as_ref().ok());
        if n.kind == NodeKind::Directory && !meta.as_ref().is_ok_and(dataless) {
            match crate::platform::is_package(&path) {
                Ok(value) => n.is_package = value,
                Err(error) => event.problems.push(Problem::io(path.clone(), error)),
            }
        }
        let mut child = None;
        let problem = match meta {
            Err(e) => Some(Problem::io(path.clone(), e)),
            Ok(st)
                if (dataless(&st) && n.kind == NodeKind::Directory)
                    || n.cloud == CloudState::LegacyPlaceholder =>
            {
                n.logical = 0;
                n.allocated = 0;
                n.coverage = Coverage::Unknown;
                Some(Problem::note(
                    path.clone(),
                    "Cloud metadata only: contents unavailable without materialization / cloud: obsah nedostupný bez stažení",
                    false,
                ))
            }
            Ok(st) if st.st_dev != device => {
                n.coverage = Coverage::Unknown;
                n.logical = 0;
                n.allocated = 0;
                Some(Problem::note(
                    path.clone(),
                    "Volume boundary skipped / jiný svazek vynechán",
                    true,
                ))
            }
            Ok(st) if n.kind == NodeKind::Directory => {
                match Directory::open(directory.fd(), &name, Some(device)) {
                    Ok((d, actual)) if actual.st_ino == st.st_ino && actual.st_dev == st.st_dev => {
                        child = Some((d, actual));
                        None
                    }
                    Ok(_) => Some(Problem::note(
                        path.clone(),
                        "Directory changed during scan / složka se během skenu změnila",
                        false,
                    )),
                    Err(e) => Some(Problem::io(path.clone(), e)),
                }
            }
            Ok(_) if n.kind == NodeKind::Special => Some(Problem::note(
                path.clone(),
                "Special object: metadata only / zvláštní objekt: jen metadata",
                false,
            )),
            Ok(_) => None,
        };
        if let Some(problem) = problem {
            if n.coverage == Coverage::Complete {
                n.coverage = if n.kind == NodeKind::Directory {
                    Coverage::Unknown
                } else {
                    Coverage::Partial
                };
            }
            event.problems.push(problem);
        }
        if child.is_some() {
            n.coverage = Coverage::Partial;
        }
        nodes.push(n);
        nodes[parent].children.push(id);
        // Geometric growth bounds total copying; time gating caps update frequency.
        if nodes.len() >= published_count.saturating_mul(2).max(256)
            && published.elapsed() >= Duration::from_millis(200)
        {
            event.revision += 1;
            let data = snapshot(&nodes, request.run_id);
            output.lock().unwrap().publish(Event {
                run_id: request.run_id,
                revision: event.revision,
                state: State::Running,
                data: Some(data),
                problems: event.problems.clone(),
            });
            published = Instant::now();
            published_count = nodes.len();
        }
        if let Some((d, st)) = child {
            stack.push((id, d, st));
        }
    }
    if event.state == State::Cancelled {
        nodes[0].coverage = Coverage::Partial;
    }
    let source_available = match root_directory(&root, &request) {
        Ok((_, current)) if current.st_dev == st.st_dev && current.st_ino == st.st_ino => true,
        result => {
            let problem = match result {
                Err(error) => Problem::io(root, error),
                Ok(_) => Problem::note(
                    root,
                    "Source replaced during scan / zdroj byl během skenu nahrazen",
                    false,
                ),
            };
            event.problems.push(problem);
            nodes[0].coverage = Coverage::Partial;
            false
        }
    };
    aggregate(&mut nodes);
    if event.state != State::Cancelled && !source_available {
        event.state = State::Unavailable;
    } else if event.state != State::Cancelled {
        event.state = if event.problems.is_empty() {
            State::Complete
        } else {
            State::Limited
        };
    }
    event.revision += 1;
    event.data = Some(Arc::new(Dataset::scanned(nodes, request.run_id)));
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::{
            fs::{MetadataExt, PermissionsExt, symlink},
            net::UnixListener,
        },
        time::{Duration, Instant},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "bytescape-scan-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            // macOS /var is an alias; fixtures use a verified physical ancestor.
            Self(fs::canonicalize(path).unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn finish(root: &Path) -> Event {
        finish_request(Request::new(root.into(), 41))
    }
    fn finish_request(request: Request) -> Event {
        let scan = Scan::start(request);
        let deadline = Instant::now() + Duration::from_secs(10);
        let event = loop {
            match scan.poll() {
                Ok(e) if e.state != State::Running => break e,
                Ok(_) => continue,
                Err(mpsc::TryRecvError::Empty) if Instant::now() < deadline => {
                    std::thread::yield_now()
                }
                _ => panic!("scan did not produce its terminal event"),
            }
        };
        assert_eq!(event.run_id, 41);
        assert!(event.revision > 0);
        loop {
            match scan.poll() {
                Err(mpsc::TryRecvError::Disconnected) => break,
                Err(mpsc::TryRecvError::Empty) if Instant::now() < deadline => {
                    std::thread::yield_now()
                }
                _ => panic!("expected exactly one terminal event"),
            }
        }
        event
    }
    #[test]
    fn slow_consumer_releases_superseded_revisions_and_receives_final_result() {
        let f = Fixture::new();
        for i in 0..1024 {
            fs::write(f.0.join(format!("file-{i:04}")), b"abc").unwrap();
        }
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let mut request = Request::new(f.0.clone(), 41);
        request.metadata_pause = Some((entered, release.clone()));
        request.metadata_delay = Duration::from_millis(1);
        let scan = Scan::start(request);
        let deadline = Instant::now() + Duration::from_secs(15);
        let first = loop {
            assert!(Instant::now() < deadline);
            if let Ok(event) = scan.poll() {
                break event.data.unwrap();
            }
            std::thread::yield_now();
        };
        let superseded = Arc::downgrade(&first);
        drop(first);
        release.store(true, Ordering::SeqCst);
        // Do not poll: a slow consumer must not keep a queue of old full trees.
        while superseded.upgrade().is_some() {
            assert!(Instant::now() < deadline);
            assert!(
                !scan.worker_finished(),
                "old snapshot retained until scan completion"
            );
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(
            !scan.worker_finished(),
            "superseded revision must be released before the terminal result"
        );
        while !scan.worker_finished() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(2));
        }
        let final_event = scan.poll().unwrap();
        assert_eq!(final_event.state, State::Complete);
        let data = final_event.data.unwrap();
        assert_eq!(data.files, 1024);
        assert_eq!(data.nodes[0].logical, 3072);
        assert!(scan.poll().is_err());
    }
    #[test]
    fn missing_source_has_localized_primary_reason_and_original_system_detail() {
        let fixture = Fixture::new();
        let missing = fixture.0.join("missing");
        let event = finish(&missing);
        assert_eq!(event.state, State::Failed);
        let problem = event.problems.iter().find(|p| p.path == missing).unwrap();
        assert_eq!(
            problem.description(true),
            "Položka již neexistuje nebo není dostupná."
        );
        assert_eq!(
            problem.description(false),
            "Item no longer exists or is unavailable."
        );
        assert!(!problem.reason.is_empty());
    }
    #[test]
    fn hard_links_have_one_owner_and_preserve_all_paths_in_every_revision() {
        let f = Fixture::new();
        for folder in ["z", "a"] {
            fs::create_dir(f.0.join(folder)).unwrap();
        }
        fs::write(f.0.join("z/original"), b"1234567").unwrap();
        fs::hard_link(f.0.join("z/original"), f.0.join("a/alias")).unwrap();
        fs::write(f.0.join("same-length"), b"7654321").unwrap();
        fs::write(f.0.join("empty"), b"").unwrap();
        for i in 0..300 {
            fs::hard_link(f.0.join("z/original"), f.0.join(format!("alias-{i}"))).unwrap();
        }
        symlink(&f.0, f.0.join("cycle")).unwrap();
        let mut request = Request::new(f.0.clone(), 41);
        request.metadata_delay = Duration::from_millis(1);
        let scan = Scan::start(request);
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut saw_progress = false;
        loop {
            assert!(Instant::now() < deadline);
            let Ok(e) = scan.poll() else {
                std::thread::yield_now();
                continue;
            };
            let d = e.data.unwrap();
            let known_links: Vec<_> = d
                .nodes
                .iter()
                .filter(|n| {
                    n.name == "original" || n.name == "alias" || n.name.starts_with("alias-")
                })
                .collect();
            if !known_links.is_empty() {
                assert_eq!(known_links.iter().map(|n| n.logical).sum::<u64>(), 7);
                let owner = known_links.iter().min_by_key(|n| &n.native_path).unwrap();
                assert_eq!(owner.logical, 7);
                for n in &known_links {
                    assert_eq!(n.reported_size.unwrap().0, 7);
                    if let Some(owner_id) = n.hard_link_owner {
                        assert_eq!(n.logical, 0);
                        assert_eq!(n.allocated, 0);
                        assert_eq!(d.nodes[owner_id].native_path, owner.native_path);
                    }
                }
            }
            for (id, n) in d.nodes.iter().enumerate() {
                if n.kind != NodeKind::Directory {
                    continue;
                }
                for limit in [1, 3, 48] {
                    let entries =
                        crate::model::contents(&d, crate::model::Scope::folder(id), limit, true);
                    assert_eq!(entries.iter().map(|e| e.logical).sum::<u64>(), n.logical);
                    assert_eq!(
                        entries.iter().map(|e| e.allocated).sum::<u64>(),
                        n.allocated
                    );
                    let mut members = Vec::new();
                    for entry in entries {
                        match entry.key {
                            crate::model::Choice::Node(id) => members.push(id),
                            crate::model::Choice::Group(scope) => {
                                members.extend(crate::model::members(&d, scope).iter().copied())
                            }
                        }
                    }
                    members.sort_unstable();
                    let mut expected = n.children.clone();
                    expected.sort_unstable();
                    assert_eq!(members, expected);
                }
                assert_eq!(d.children(id, true).len(), n.children.len());
            }
            if e.state == State::Running {
                saw_progress |= known_links.len() > 1;
                continue;
            }
            assert_eq!(e.state, State::Complete);
            assert!(saw_progress);
            assert_eq!(known_links.len(), 302);
            let cycle = d.nodes.iter().find(|n| n.name == "cycle").unwrap();
            assert_eq!(cycle.kind, NodeKind::Symlink);
            assert!(cycle.children.is_empty());
            assert_eq!(d.nodes[0].logical, 14 + cycle.logical);
            let blocks = fs::metadata(f.0.join("z/original")).unwrap().blocks()
                + fs::metadata(f.0.join("same-length")).unwrap().blocks();
            assert_eq!(d.nodes[0].allocated, blocks * 512 + cycle.allocated);
            let version = std::sync::atomic::AtomicU64::new(1);
            let result = crate::model::search(&d, "alias", None, 1, &version).unwrap();
            assert_eq!(result.hits.len(), 301);
            break;
        }
    }
    #[test]
    fn empty_hard_links_remain_searchable_and_explorable() {
        let f = Fixture::new();
        fs::write(f.0.join("zero-a"), b"").unwrap();
        fs::hard_link(f.0.join("zero-a"), f.0.join("zero-b")).unwrap();
        let e = finish(&f.0);
        assert_eq!(e.state, State::Complete);
        let d = e.data.unwrap();
        assert_eq!(d.nodes[0].logical, 0);
        let owner = d.nodes.iter().position(|n| n.name == "zero-a").unwrap();
        let alias = d.nodes.iter().position(|n| n.name == "zero-b").unwrap();
        assert_eq!(d.nodes[alias].hard_link_owner, Some(owner));
        assert_eq!(d.nodes[alias].reported_size, Some((0, 0)));
        let entries = crate::model::contents(&d, crate::model::Scope::ROOT, 1, true);
        assert_eq!(entries.len(), 2);
        let crate::model::Choice::Group(scope) = entries[1].key else {
            panic!("missing zero-size aggregate")
        };
        let tail = crate::model::members(&d, scope);
        assert_eq!(tail.len(), 1);
        assert!(tail[0] == alias || tail[0] == owner);
        assert_ne!(entries[0].key, crate::model::Choice::Node(tail[0]));
        let version = std::sync::atomic::AtomicU64::new(1);
        let result = crate::model::search(&d, "zero-b", None, 1, &version).unwrap();
        assert_eq!(result.hits.as_slice(), &[alias]);
        assert_eq!(result.logical, 0);
        assert!(d.copy_path(alias).unwrap().ends_with("zero-b"));
    }
    #[test]
    fn progressive_revisions_are_consistent_and_finish_once() {
        let f = Fixture::new();
        fs::create_dir(f.0.join("nested")).unwrap();
        for i in 0..300 {
            fs::write(f.0.join("nested").join(i.to_string()), b"abc").unwrap();
        }
        let mut request = Request::new(f.0.clone(), 41);
        request.metadata_delay = Duration::from_millis(1);
        let scan = Scan::start(request);
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut revision = 0;
        let mut partial_files = false;
        loop {
            assert!(Instant::now() < deadline);
            if let Ok(event) = scan.poll() {
                assert!(event.revision > revision);
                revision = event.revision;
                let d = event.data.unwrap();
                for n in &d.nodes {
                    if n.kind == NodeKind::Directory {
                        assert_eq!(
                            n.logical,
                            n.children.iter().map(|&c| d.nodes[c].logical).sum()
                        );
                        assert_eq!(
                            n.allocated,
                            n.children.iter().map(|&c| d.nodes[c].allocated).sum()
                        );
                    }
                }
                if event.state != State::Running {
                    assert_eq!(event.state, State::Complete);
                    assert_eq!(d.nodes[0].logical, 900);
                    break;
                }
                partial_files |= d.files > 0 && d.files < 300;
                assert_eq!(d.nodes[0].coverage, Coverage::Partial);
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(partial_files);
        assert!(scan.poll().is_err());
    }
    #[test]
    fn cancellation_freezes_progress_while_metadata_is_blocked() {
        let f = Fixture::new();
        fs::write(f.0.join("slow"), b"123").unwrap();
        let entered = Arc::new(AtomicBool::new(false));
        let release = Arc::new(AtomicBool::new(false));
        let mut request = Request::new(f.0.clone(), 72);
        request.metadata_pause = Some((entered.clone(), release.clone()));
        let scan = Scan::start(request);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !entered.load(Ordering::SeqCst) {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        let progress = scan.poll().unwrap();
        assert_eq!(progress.state, State::Running);
        let before = Instant::now();
        scan.cancel();
        let terminal = scan.poll().unwrap();
        assert_eq!(terminal.state, State::Cancelled);
        assert!(before.elapsed() < Duration::from_secs(1));
        assert_eq!(
            terminal.data.unwrap().nodes.len(),
            progress.data.unwrap().nodes.len()
        );
        assert!(!scan.worker_finished());
        release.store(true, Ordering::SeqCst);
        while !scan.worker_finished() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        assert!(scan.poll().is_err(), "late result must be ignored");
        assert_eq!(finish(&f.0).data.unwrap().nodes[0].logical, 3);
    }
    #[test]
    fn real_tree_metadata_hierarchy_identity_and_shared_views() {
        let f = Fixture::new();
        fs::create_dir(f.0.join("Prázdná složka")).unwrap();
        fs::create_dir(f.0.join("Vnořená")).unwrap();
        fs::write(f.0.join("Vnořená/žluťoučký.txt"), b"12345").unwrap();
        fs::write(f.0.join(".hidden"), b"abc").unwrap();
        fs::write(f.0.join("empty"), b"").unwrap();
        fs::write(f.0.join("L".repeat(230)), b"12").unwrap();
        let sparse = fs::File::create(f.0.join("sparse")).unwrap();
        sparse.set_len(1_048_576).unwrap();
        let mut raw_count = 0;
        for name in [b"raw-\xff".as_slice(), b"raw-\xfe".as_slice()] {
            match fs::write(f.0.join(OsStr::from_bytes(name)), b"x") {
                Ok(()) => raw_count += 1,
                Err(e) => {
                    assert_eq!(e.raw_os_error(), Some(libc::EILSEQ));
                    eprintln!(
                        "UNVERIFIED native non-UTF8 filename: this volume rejects it with EILSEQ"
                    );
                }
            }
        }
        let event = finish(&f.0);
        assert_eq!(event.state, State::Complete);
        let d = event.data.unwrap();
        assert_eq!(d.run_id, Some(41));
        assert_eq!(d.files, 5 + raw_count);
        assert_eq!(d.nodes[0].logical, 1_048_586 + raw_count as u64);
        assert_eq!(d.nodes.len(), 8 + raw_count);
        let allocated: u64 = d
            .nodes
            .iter()
            .filter(|n| n.kind == NodeKind::File)
            .map(|n| {
                fs::symlink_metadata(n.native_path.as_ref().unwrap())
                    .unwrap()
                    .blocks()
                    * 512
            })
            .sum();
        assert_eq!(d.nodes[0].allocated, allocated);
        for (id, n) in d.nodes.iter().enumerate() {
            assert_eq!(n.coverage, Coverage::Complete);
            assert!(n.native_path.as_ref().unwrap().starts_with(&f.0));
            if n.kind == NodeKind::Directory {
                assert_eq!(
                    n.logical,
                    n.children.iter().map(|&c| d.nodes[c].logical).sum()
                );
                assert_eq!(
                    n.allocated,
                    n.children.iter().map(|&c| d.nodes[c].allocated).sum()
                );
                for &c in &n.children {
                    assert_eq!(d.nodes[c].parent, Some(id));
                }
                for limit in [1, 3, 48] {
                    let entries =
                        crate::model::contents(&d, crate::model::Scope::folder(id), limit, true);
                    assert_eq!(entries.iter().map(|e| e.logical).sum::<u64>(), n.logical);
                    assert_eq!(
                        entries.iter().map(|e| e.allocated).sum::<u64>(),
                        n.allocated
                    );
                }
            }
        }
        let raw: Vec<_> = d
            .nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.name.starts_with("raw-"))
            .collect();
        assert_eq!(raw.len(), raw_count);
        if raw_count == 2 {
            assert_ne!(raw[0].1.native_path, raw[1].1.native_path);
            assert!(d.copy_path(raw[0].0).is_none());
        }
        let empty = d.nodes.iter().find(|n| n.name == "Prázdná složka").unwrap();
        assert_eq!(empty.kind, NodeKind::Directory);
        assert!(empty.children.is_empty());
        let version = std::sync::atomic::AtomicU64::new(1);
        let found = crate::model::search(&d, "Prázdná", None, 1, &version).unwrap();
        assert_eq!(found.files, 0);
        assert_eq!(found.hits.len(), 1);
    }
    #[test]
    #[cfg(target_os = "macos")]
    fn dataless_metadata_is_kept_without_entering_unavailable_directories() {
        let f = Fixture::new();
        let folder = f.0.join("remote-folder");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("must-not-appear"), b"remote").unwrap();
        let file = f.0.join("remote.txt");
        fs::write(&file, b"1234567").unwrap();
        let mut request = Request::new(f.0.clone(), 41);
        request.dataless_paths = vec![folder.clone(), file.clone()];
        request.before_metadata = Some(Arc::new(|_| {
            unsafe extern "C" {
                fn getiopolicy_np(iotype: i32, scope: i32) -> i32;
            }
            // Independent system readback on the actual worker thread, not a mock.
            assert_eq!(unsafe { getiopolicy_np(3, 1) }, 1);
        }));
        let e = finish_request(request);
        assert_eq!(e.state, State::Limited);
        let d = e.data.unwrap();
        let remote = d.nodes.iter().find(|n| n.name == "remote.txt").unwrap();
        assert_eq!(remote.cloud, CloudState::Dataless);
        assert_eq!(remote.coverage, Coverage::Complete);
        assert_eq!(remote.logical, 7);
        assert_eq!(
            remote.allocated,
            fs::metadata(&file).unwrap().blocks() * 512
        );
        let dir = d.nodes.iter().find(|n| n.name == "remote-folder").unwrap();
        assert_eq!(dir.cloud, CloudState::Dataless);
        assert_eq!(dir.coverage, Coverage::Unknown);
        assert!(dir.children.is_empty());
        assert!(!d.nodes.iter().any(|n| n.name == "must-not-appear"));
        assert_eq!(d.nodes[0].logical, 7);
        assert_eq!(d.nodes[0].coverage, Coverage::Partial);
        let mut request = Request::new(folder.clone(), 41);
        request.dataless_paths = vec![folder];
        let root = finish_request(request);
        assert_eq!(root.state, State::Limited);
        let data = root.data.unwrap();
        assert_eq!(data.nodes.len(), 1);
        assert_eq!(data.nodes[0].coverage, Coverage::Unknown);
    }
    #[test]
    fn available_cloud_branches_keep_metadata_and_legacy_stubs_stay_unknown() {
        let f = Fixture::new();
        fs::create_dir_all(f.0.join("Library/CloudStorage/Provider/nested")).unwrap();
        fs::write(
            f.0.join("Library/CloudStorage/Provider/nested/cached.txt"),
            b"1234567",
        )
        .unwrap();
        fs::write(f.0.join(".remote.txt.icloud"), b"not the remote file size").unwrap();
        let e = finish(&f.0);
        assert_eq!(e.state, State::Limited);
        let d = e.data.unwrap();
        let cached = d.nodes.iter().find(|n| n.name == "cached.txt").unwrap();
        assert_eq!(cached.cloud, CloudState::Cached);
        assert_eq!(cached.reported_size.unwrap().0, 7);
        assert_eq!(cached.coverage, Coverage::Complete);
        let stub = d
            .nodes
            .iter()
            .find(|n| n.name == ".remote.txt.icloud")
            .unwrap();
        assert_eq!(stub.cloud, CloudState::LegacyPlaceholder);
        assert_eq!(stub.coverage, Coverage::Unknown);
        assert_eq!(stub.reported_size, None);
        assert_eq!(d.nodes[0].logical, 7);
        assert_eq!(d.nodes[0].coverage, Coverage::Partial);
        let nested = finish(&f.0.join("Library/CloudStorage/Provider"));
        assert_eq!(nested.state, State::Complete);
        assert_eq!(nested.data.unwrap().nodes[0].logical, 7);
    }
    #[test]
    #[cfg(target_os = "macos")]
    fn packages_are_summarized_and_their_real_children_are_already_available() {
        let f = Fixture::new();
        fs::create_dir_all(f.0.join("Test.app/Contents/Resources")).unwrap();
        fs::write(f.0.join("Test.app/Contents/Resources/data.txt"), b"1234567").unwrap();
        fs::write(f.0.join("ordinary.txt"), b"abc").unwrap();
        let e = finish(&f.0);
        assert_eq!(e.state, State::Complete);
        let d = e.data.unwrap();
        let id = d.nodes.iter().position(|n| n.name == "Test.app").unwrap();
        assert!(d.nodes[id].is_package);
        assert_eq!(d.nodes[id].kind, NodeKind::Directory);
        assert_eq!(d.nodes[id].logical, 7);
        assert_eq!(d.nodes[0].logical, 10);
        assert_eq!(d.nodes[d.children(id, false)[0]].name, "Contents");
        let scope = crate::model::Scope { root: id, start: 0 };
        assert_eq!(crate::model::members(&d, scope).len(), 1);
        let nested = finish(&f.0.join("Test.app"));
        assert!(nested.data.as_ref().unwrap().nodes[0].is_package);
        assert_eq!(nested.data.unwrap().nodes[0].logical, 7);
    }
    #[test]
    fn links_special_objects_and_errors_do_not_block_available_cloud_metadata() {
        let f = Fixture::new();
        let outside = Fixture::new();
        fs::write(outside.0.join("must-not-appear"), b"outside").unwrap();
        symlink(&outside.0, f.0.join("outside-link")).unwrap();
        symlink(&f.0, f.0.join("cycle")).unwrap();
        fs::write(f.0.join("file"), b"1234567").unwrap();
        fs::hard_link(f.0.join("file"), f.0.join("hard-link")).unwrap();
        fs::create_dir_all(f.0.join("Library/CloudStorage/provider")).unwrap();
        fs::write(
            f.0.join("Library/CloudStorage/provider/must-not-appear"),
            b"cloud",
        )
        .unwrap();
        let fifo = CString::new(f.0.join("pipe").as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        let _socket = UnixListener::bind(f.0.join("socket")).unwrap();
        let e = finish(&f.0);
        assert_eq!(e.state, State::Limited);
        assert!(!e.problems.iter().any(|p| p.reason.contains("Hard links")));
        let d = e.data.unwrap();
        assert_eq!(
            d.nodes
                .iter()
                .filter(|n| n.name == "must-not-appear")
                .count(),
            1
        );
        assert!(d.nodes.iter().any(|n| n.name == "provider"));
        for name in ["cycle", "outside-link"] {
            let n = d.nodes.iter().find(|n| n.name == name).unwrap();
            assert_eq!(n.kind, NodeKind::Symlink);
            assert!(n.children.is_empty());
        }
        for name in ["pipe", "socket"] {
            assert_eq!(
                d.nodes.iter().find(|n| n.name == name).unwrap().kind,
                NodeKind::Special
            );
        }
        assert_eq!(d.nodes[0].coverage, Coverage::Partial);
        assert_eq!(finish(&f.0.join("missing")).state, State::Failed);
        assert_eq!(finish(&f.0.join("file")).state, State::Failed);
        assert_eq!(finish(&f.0.join("outside-link")).state, State::Failed);
        assert_eq!(
            finish(&f.0.join("outside-link/subfolder")).state,
            State::Failed
        );
        assert_eq!(
            finish(&f.0.join("Library/CloudStorage")).state,
            State::Complete
        );
    }
    #[test]
    fn unreadable_directory_is_partial_and_not_a_successful_empty_folder() {
        let f = Fixture::new();
        let denied = f.0.join("denied");
        fs::create_dir(&denied).unwrap();
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o0)).unwrap();
        assert!(
            fs::read_dir(&denied).is_err(),
            "test requires an unprivileged process"
        );
        let e = finish(&f.0);
        let root_failure = finish(&denied);
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(root_failure.state, State::Failed);
        assert!(root_failure.data.is_none());
        assert!(
            root_failure
                .problems
                .iter()
                .any(|p| p.permission_denied && p.advice(true).is_some())
        );
        assert_eq!(e.state, State::Limited);
        assert!(e.problems.iter().any(|p| p.path == denied
            && p.permission_denied
            && p.advice(true).is_some()
            && p.advice(false).is_some()));
        let d = e.data.unwrap();
        assert_eq!(
            d.nodes
                .iter()
                .find(|n| n.name == "denied")
                .unwrap()
                .coverage,
            Coverage::Unknown
        );
        assert_eq!(d.nodes[0].coverage, Coverage::Partial);
        let denied = d.nodes.iter().position(|n| n.name == "denied").unwrap();
        let entry = crate::model::entry(&d, crate::model::Choice::Node(denied), true);
        assert_eq!(entry.coverage.format(entry.logical), "?");
    }
    #[test]
    fn detected_directory_change_preserves_independent_branch() {
        let f = Fixture::new();
        fs::create_dir(f.0.join("changing")).unwrap();
        fs::write(f.0.join("changing/trigger"), b"123").unwrap();
        fs::write(f.0.join("independent"), b"12345").unwrap();
        let mut request = Request::new(f.0.clone(), 41);
        request.before_metadata = Some(Arc::new(|path| {
            if path.file_name().unwrap() == "trigger" {
                fs::write(path.parent().unwrap().join("created-during-scan"), b"x").unwrap();
            }
        }));
        let e = finish_request(request);
        assert_eq!(e.state, State::Limited);
        assert!(e.problems.iter().any(|p| p.reason.contains("changed")));
        let d = e.data.unwrap();
        assert_eq!(d.nodes[0].coverage, Coverage::Partial);
        assert!(d.nodes.iter().any(|n| n.name == "independent"
            && n.logical == 5
            && n.coverage == Coverage::Complete));
    }
    #[test]
    fn lost_source_retains_discovered_sizes_and_paths() {
        let f = Fixture::new();
        let moved = f.0.with_extension("moved");
        fs::write(f.0.join("retained"), b"12345").unwrap();
        let original = f.0.clone();
        let destination = moved.clone();
        let mut request = Request::new(original.clone(), 41);
        request.before_metadata = Some(Arc::new(move |_| {
            fs::rename(&original, &destination).unwrap();
        }));
        let e = finish_request(request);
        fs::rename(&moved, &f.0).unwrap();
        assert_eq!(e.state, State::Unavailable);
        assert!(e.problems.iter().any(|p| p.path == f.0));
        let d = e.data.unwrap();
        assert_eq!(d.nodes[0].logical, 5);
        assert_eq!(d.nodes[0].coverage, Coverage::Partial);
        assert_eq!(
            d.nodes[1].native_path.as_ref().unwrap(),
            &f.0.join("retained")
        );
        assert_eq!(d.nodes[1].coverage, Coverage::Complete);
    }
    #[test]
    fn vanished_child_is_unknown_while_sibling_and_shared_views_remain_available() {
        let f = Fixture::new();
        fs::create_dir(f.0.join("nested")).unwrap();
        fs::write(f.0.join("nested/vanishing"), b"gone").unwrap();
        fs::write(f.0.join("nested/empty"), b"").unwrap();
        fs::write(f.0.join("independent"), b"1234567").unwrap();
        let mut request = Request::new(f.0.clone(), 41);
        request.before_metadata = Some(Arc::new(|path| {
            if path.file_name().unwrap() == "vanishing" {
                fs::remove_file(path).unwrap();
            }
        }));
        let e = finish_request(request);
        assert_eq!(e.state, State::Limited);
        assert!(e.problems.iter().any(|p| p.path.ends_with("vanishing")));
        let d = e.data.unwrap();
        assert_eq!(d.nodes[0].logical, 7);
        let version = std::sync::atomic::AtomicU64::new(1);
        let result = crate::model::search(&d, "vanishing", None, 1, &version).unwrap();
        assert_eq!(result.coverage, Coverage::Partial);
        assert_ne!(result.coverage.format(result.logical), "0 B");
        for (id, n) in d.nodes.iter().enumerate() {
            let entry = crate::model::entry(&d, crate::model::Choice::Node(id), false);
            assert_eq!(entry.coverage, n.coverage);
            match n.name.as_str() {
                "vanishing" => assert_eq!(entry.coverage.format(entry.logical), "?"),
                "empty" => assert_eq!(entry.coverage.format(entry.logical), "0 B"),
                "independent" => assert_eq!(entry.logical, 7),
                _ => assert_eq!(entry.coverage, Coverage::Partial),
            }
            if n.kind == NodeKind::Directory {
                let entries = crate::model::contents(&d, crate::model::Scope::folder(id), 1, false);
                assert_eq!(entries.iter().map(|e| e.logical).sum::<u64>(), n.logical);
                assert_eq!(
                    entries.iter().map(|e| e.allocated).sum::<u64>(),
                    n.allocated
                );
            }
        }
        let missing = finish(&f.0.join("missing"));
        assert_eq!(missing.state, State::Failed);
        assert!(missing.data.is_none());
    }
    #[test]
    fn metadata_volume_boundary_is_skipped_but_available_case_variant_cloud_is_scanned() {
        let f = Fixture::new();
        let boundary = f.0.join("volume");
        fs::create_dir(&boundary).unwrap();
        fs::write(boundary.join("must-not-appear"), b"secret").unwrap();
        let mut request = Request::new(f.0.clone(), 41);
        request.metadata_override = Some((
            boundary.clone(),
            fs::metadata(&f.0).unwrap().dev().wrapping_add(1) as libc::dev_t,
        ));
        let e = finish_request(request);
        assert_eq!(e.state, State::Limited);
        assert!(
            e.problems
                .iter()
                .any(|p| p.path == boundary && p.volume_boundary && p.reason.contains("Volume"))
        );
        let d = e.data.unwrap();
        assert!(!d.nodes.iter().any(|n| n.name == "must-not-appear"));
        assert_eq!(d.nodes[1].coverage, Coverage::Unknown);
        fs::create_dir_all(f.0.join("library/cloudstorage/provider")).unwrap();
        fs::write(
            f.0.join("library/cloudstorage/provider/must-not-appear"),
            b"secret",
        )
        .unwrap();
        let e = finish(&f.0);
        assert_eq!(e.state, State::Complete);
        assert!(e.data.unwrap().nodes.iter().any(|n| n.name == "provider"));
        assert_eq!(
            finish(&f.0.join("library/cloudstorage")).state,
            State::Complete
        );
    }
}
