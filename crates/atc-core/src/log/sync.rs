//! Synchronous shared-board coordination. Every transport belongs to one call and one deadline.

use std::io::{Read, Seek, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::counter::{self, Coordination, SEQ_REF};
use super::{Error, Event, EventId, Kind, Result, Store, chain, ref_target, wall_clock};

#[derive(Clone, Copy)]
pub enum Touch {
    Ordinary,
    Number,
    Enroll,
}

#[derive(Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    blocked: Option<String>,
    #[serde(default)]
    detached: bool,
    pub endpoint: String,
    pub attempted: i64,
    pub result: Option<String>,
    pub enrolled: Option<String>,
    enrollment: Option<Enrollment>,
    pending_remote: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Enrollment {
    domain: String,
    flights: Vec<EventId>,
}

impl Store {
    /// Ordinary mutations resolve against a local snapshot and synchronize after appending their wire-ID-bound events.
    pub fn append_synced(&self, kinds: Vec<Kind>) -> Result<Vec<EventId>> {
        let ids = self.append(kinds)?;
        self.touch(Touch::Ordinary)?;
        Ok(ids)
    }

    /// Establish the local numbering context before new filings, without paying a separate network deadline.
    pub fn prepare_filing(&self) -> Result<()> {
        if let Some(remote) = self.remote() {
            let state = self.sync_state()?;
            if let Some(enrollment) = state.enrollment
                && state.enrolled.as_deref() != Some(&enrollment.domain)
            {
                return Err(Error::Enrollment {
                    count: enrollment.flights.len(),
                    remote,
                });
            }
        }
        let deadline = Instant::now() + self.sync_duration("numberTimeout", 10);
        self.allocation_deadline.set(Some(deadline));
        self.snapshot()?;
        if self.remote().is_some() && self.counter()?.is_none() {
            self.coordinate(deadline)?.initialize()?;
        }
        Ok(())
    }
    pub fn remote(&self) -> Option<String> {
        self.repo
            .config_snapshot()
            .string("tower.remote")
            .map(|v| v.to_string())
            .filter(|v| !v.is_empty())
    }

    pub fn sync_interval(&self) -> Duration {
        self.sync_duration("syncInterval", 30)
    }

    fn sync_duration(&self, key: &str, default: u64) -> Duration {
        self.config()
            .read(crate::config::lookup(key).expect("registered"))
            .value
            .as_deref()
            .and_then(crate::config::parse_window)
            .unwrap_or(Duration::from_secs(default))
    }

    /// Local snapshot for input resolution. Migration is local and precedes the snapshot; no remote touch can retarget input.
    pub fn snapshot(&self) -> Result<crate::board::Fold> {
        let deadline = self
            .allocation_deadline
            .get()
            .unwrap_or_else(|| Instant::now() + self.sync_duration("numberTimeout", 10));
        if self.remote().is_none() {
            let held = self.coordinate(deadline)?;
            held.migrate()?;
        } else if self.counter()?.is_none()
            && self
                .read_all()?
                .iter()
                .any(|e| matches!(e.kind, Kind::Filed { .. }))
        {
            self.coordinate(deadline)?.bootstrap_legacy()?;
        }
        self.current()
    }

    /// Render available local state without acquiring another operation budget after a touch.
    pub fn current(&self) -> Result<crate::board::Fold> {
        let value = self.counter()?.map_or(0, |counter| counter.value);
        Ok(crate::board::fold_numbered(&self.read_all()?, value))
    }

    pub fn sync_state(&self) -> Result<State> {
        match std::fs::read(self.repo.common_dir().join("tower/sync.json")) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|err| refuse(format!("invalid sync state: {err}"))),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(State::default()),
            Err(err) => Err(Error::repo(err)),
        }
    }

    /// Network failure preserves local work. Structural failures and enrollment gates refuse explicitly.
    pub fn touch(&self, mode: Touch) -> Result<()> {
        let duration = match mode {
            Touch::Ordinary => self.sync_duration("syncTimeout", 3),
            _ => self.sync_duration("numberTimeout", 10),
        };
        let deadline = if matches!(mode, Touch::Number) {
            self.allocation_deadline
                .get()
                .unwrap_or_else(|| Instant::now() + duration)
        } else {
            Instant::now() + duration
        };
        let result = (|| {
            let held = self.coordinate(deadline)?;
            let Some(remote) = self.remote() else {
                return held.migrate();
            };
            crate::config::validate(
                crate::config::lookup("remote").expect("registered"),
                &remote,
            )
            .map_err(|err| refuse(err.to_string()))?;
            let endpoint = format!(
                "{remote}\n{}",
                self.repo
                    .config_snapshot()
                    .string(format!("remote.{remote}.url").as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            );
            let mut state = self.sync_state()?;
            if matches!(mode, Touch::Ordinary)
                && state.endpoint == endpoint
                && wall_clock().saturating_sub(state.attempted)
                    < self.sync_interval().as_secs() as i64
            {
                if let Some(enrollment) = &state.enrollment
                    && state.enrolled.as_deref() != Some(&enrollment.domain)
                {
                    return Err(Error::Enrollment {
                        count: enrollment.flights.len(),
                        remote,
                    });
                }
                if let Some(blocked) = &state.blocked {
                    return Err(refuse(blocked));
                }
                return Ok(());
            }
            state.attempted = wall_clock();
            state.endpoint = endpoint.clone();
            held.save(&state)?;
            let result = loop {
                let result = held.synchronize(
                    &remote,
                    &endpoint,
                    &mut state,
                    deadline,
                    matches!(mode, Touch::Enroll),
                );
                if !matches!(mode, Touch::Ordinary)
                    && matches!(result, Err(Error::Transport { .. }))
                {
                    let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                        break Err(Error::Deadline);
                    };
                    std::thread::sleep(remaining.min(Duration::from_millis(100)));
                    continue;
                }
                break result;
            };
            state.result = Some(match &result {
                Ok(()) => "ok".to_string(),
                Err(err) => err.to_string(),
            });
            state.blocked = match &result {
                Err(Error::Transport { .. } | Error::Deadline) | Ok(()) => None,
                Err(err) => Some(err.to_string()),
            };
            held.save(&state)?;
            result
        })();
        if matches!(mode, Touch::Number) {
            self.allocation_deadline.set(None);
        }
        match result {
            Err(Error::Transport { .. } | Error::Deadline) if !matches!(mode, Touch::Enroll) => {
                Ok(())
            }
            other => other,
        }
    }
}

impl Coordination<'_> {
    fn bootstrap_legacy(&self) -> Result<()> {
        if self.store.counter()?.is_some() {
            return Ok(());
        }
        let fold = crate::board::fold(&self.store.read_all()?);
        if fold.flights.iter().any(|f| f.global_number.is_some()) {
            return Ok(());
        }
        let mut flights: Vec<_> = fold.flights.iter().map(|f| f.id.clone()).collect();
        if flights.is_empty() {
            return Ok(());
        }
        if flights.iter().all(|f| f.writer == flights[0].writer) {
            flights.sort_by_key(|id| id.seq);
        }
        let root = self.initialize()?;
        let reservation = self.prepare(&root, &flights)?;
        counter::move_ref(&self.store.repo, SEQ_REF, reservation.tip, Some(root.tip))?;
        self.record(&reservation)
    }
    fn migrate(&self) -> Result<()> {
        let mut state = self.store.sync_state()?;
        if state.pending_remote.is_some() || state.enrollment.is_some() {
            return Err(refuse(
                "a shared reservation or enrollment is pending; reconnect before switching to local mode",
            ));
        }
        if state.enrolled.take().is_some() {
            state.detached = true;
            self.save(&state)?;
        }
        if self.store.writer().is_some() {
            self.recover_local()?;
        }
        let fold = crate::board::fold(&self.store.read_all()?);
        if self.store.counter()?.is_none() && fold.flights.iter().any(|f| f.global_number.is_some())
        {
            return Err(Error::Counter {
                detail: "number claims exist but refs/tower/seq is missing".to_string(),
            });
        }
        let mut flights: Vec<_> = fold
            .flights
            .iter()
            .filter(|f| f.global_number.is_none())
            .collect();
        if flights
            .iter()
            .all(|f| Some(&f.id.writer) == flights.first().map(|f| &f.id.writer))
        {
            flights.sort_by_key(|f| f.id.seq);
        }
        if !flights.is_empty() {
            self.claim_local(
                &flights
                    .into_iter()
                    .map(|f| f.id.clone())
                    .collect::<Vec<_>>(),
            )?;
        }
        Ok(())
    }

    fn save(&self, state: &State) -> Result<()> {
        let dir = self.store.repo.common_dir().join("tower");
        let mut file = tempfile::NamedTempFile::new_in(&dir).map_err(Error::repo)?;
        file.write_all(&serde_json::to_vec(state).map_err(Error::repo)?)
            .map_err(Error::repo)?;
        file.as_file().sync_all().map_err(Error::repo)?;
        file.persist(dir.join("sync.json")).map_err(Error::repo)?;
        Ok(())
    }

    fn synchronize(
        &self,
        remote: &str,
        endpoint: &str,
        state: &mut State,
        deadline: Instant,
        confirm: bool,
    ) -> Result<()> {
        self.bootstrap_legacy()?;
        let local = self.store.counter()?;
        let original = crate::board::fold(&self.store.read_all()?);
        let mut observed = self.fetch(remote, deadline)?;
        let remote_events = self.staged_events()?;
        let remote_fold = crate::board::fold(&remote_events);
        if let Some(pending_remote) = &state.pending_remote
            && pending_remote != endpoint
        {
            return Err(refuse(
                "an uncertain reservation belongs to another remote; reconnect to it first",
            ));
        }
        if let Some(writer) = self.store.writer()
            && let Some(tip) =
                ref_target(&self.store.repo, &format!("refs/tower/pending/{writer}"))?
        {
            let reservation = counter::reservation(&self.store.repo, tip)?;
            if state.pending_remote.is_none() {
                if let Some(local) = &local
                    && counter::contains(&self.store.repo, local.tip, tip)?
                {
                    self.record_until(&reservation, Some(deadline))?;
                } else {
                    self.clear_pending(tip)?;
                }
            } else if let Some(observed) = &observed {
                if counter::contains(&self.store.repo, observed.tip, tip)? {
                    self.record_until(&reservation, Some(deadline))?;
                } else {
                    self.clear_pending(tip)?;
                }
            } else {
                self.clear_pending(tip)?;
            }
        }
        state.pending_remote = None;
        self.save(state)?;

        if observed.is_none() {
            let counter = self.initialize()?;
            // The lease against absence resolves two writers trying to seed an empty remote.
            match self.git(
                &[
                    "push".into(),
                    "--porcelain".into(),
                    "--force-with-lease=refs/tower/seq:".into(),
                    remote.into(),
                    format!("{}:{SEQ_REF}", counter.tip),
                ],
                deadline,
            ) {
                Ok(_) => observed = Some(counter),
                Err(Error::Transport { .. }) => observed = self.fetch(remote, deadline)?,
                Err(err) => return Err(err),
            }
        }
        let mut observed = observed.ok_or_else(|| Error::Transport {
            detail: "remote counter is unavailable".to_string(),
        })?;
        let domain = observed.lineage.to_string();
        let same = !state.detached
            && (local
                .as_ref()
                .is_some_and(|c| c.lineage == observed.lineage)
                || state.enrolled.as_deref() == Some(&domain));
        if same
            && let Some(local) = &local
            && local.lineage == observed.lineage
            && !counter::contains(&self.store.repo, observed.tip, local.tip)?
        {
            return Err(refuse(
                "the remote counter rolled back or diverged from a confirmed local counter",
            ));
        }
        let moving: Vec<_> = original
            .flights
            .iter()
            .filter(|f| {
                !same
                    && f.global_number.is_some()
                    && !remote_fold
                        .flights
                        .iter()
                        .any(|other| other.id == f.id && other.global_number.is_some())
            })
            .map(|f| f.id.clone())
            .collect();
        if !same
            && !moving.is_empty()
            && state.enrollment.as_ref().is_none_or(|e| e.domain != domain)
        {
            state.enrollment = Some(Enrollment {
                domain: domain.clone(),
                flights: moving,
            });
            self.save(state)?;
            if !confirm {
                return Err(Error::Enrollment {
                    count: state.enrollment.as_ref().expect("set").flights.len(),
                    remote: remote.to_string(),
                });
            }
        }
        // A persisted enrollment is not authorization: confirmation is persisted separately by removing its gate only on this call.
        if let Some(enrollment) = &state.enrollment
            && !confirm
            && state.enrolled.as_deref() != Some(&domain)
        {
            return Err(Error::Enrollment {
                count: enrollment.flights.len(),
                remote: remote.to_string(),
            });
        }
        if state.enrollment.is_some() && confirm {
            state.enrolled = Some(domain.clone());
            self.save(state)?;
        }
        self.promote_logs(deadline)?;
        self.adopt_counter(observed.tip, deadline)?;
        loop {
            let fold = crate::board::fold(&self.store.read_all()?);
            let mut wanted = state
                .enrollment
                .as_ref()
                .map(|e| e.flights.clone())
                .unwrap_or_default();
            // Remote numbered events or recovered reservations identify completed enrollment work.
            if state.enrollment.is_some() {
                let confirmed = self.confirmed_assignments(observed.tip)?;
                wanted.retain(|id| {
                    !fold.flights.iter().any(|f| {
                        &f.id == id
                            && f.global_number
                                .is_some_and(|n| confirmed.contains(&(id.clone(), n)))
                    })
                });
            }
            for f in &fold.flights {
                if f.global_number.is_none()
                    && self.store.writer() == Some(f.id.writer.as_str())
                    && !wanted.contains(&f.id)
                {
                    wanted.push(f.id.clone());
                }
            }
            if wanted.is_empty() {
                break;
            }
            if Instant::now() >= deadline {
                return Err(Error::Deadline);
            }
            state.pending_remote = Some(endpoint.to_string());
            self.save(state)?;
            let reservation = self.prepare(&observed, &wanted)?;
            let push = self.git(
                &[
                    "push".into(),
                    "--porcelain".into(),
                    format!("--force-with-lease={SEQ_REF}:{}", observed.tip),
                    remote.into(),
                    format!("{}:{SEQ_REF}", reservation.tip),
                ],
                deadline,
            );
            match push {
                Ok(_) => {
                    counter::move_ref(
                        &self.store.repo,
                        SEQ_REF,
                        reservation.tip,
                        ref_target(&self.store.repo, SEQ_REF)?,
                    )?;
                    self.record_until(&reservation, Some(deadline))?;
                    state.pending_remote = None;
                    self.save(state)?;
                    observed = counter::inspect(&self.store.repo, reservation.tip)?;
                }
                Err(Error::Transport { .. }) => {
                    observed = self
                        .fetch(remote, deadline)?
                        .ok_or_else(|| refuse("remote counter disappeared"))?;
                    if observed.lineage.to_string() != domain {
                        return Err(refuse(
                            "remote counter changed numbering domains during allocation",
                        ));
                    }
                    if counter::contains(&self.store.repo, observed.tip, reservation.tip)? {
                        self.record_until(&reservation, Some(deadline))?;
                    } else {
                        self.clear_pending(reservation.tip)?;
                    }
                    state.pending_remote = None;
                    self.save(state)?;
                    self.adopt_counter(observed.tip, deadline)?;
                }
                Err(err) => return Err(err),
            }
        }
        state.enrolled = Some(domain);
        state.detached = false;
        state.enrollment = None;
        self.save(state)?;
        if let Some(writer) = self.store.writer() {
            let name = chain::log_ref(self.store.author(), writer);
            if ref_target(&self.store.repo, &name)?.is_some() {
                self.git(
                    &[
                        "push".into(),
                        "--porcelain".into(),
                        remote.into(),
                        format!("{name}:{name}"),
                    ],
                    deadline,
                )?;
            }
        }
        Ok(())
    }

    fn confirmed_assignments(&self, tip: gix::ObjectId) -> Result<Vec<(EventId, u64)>> {
        let root = counter::inspect(&self.store.repo, tip)?.lineage;
        let mut cursor = tip;
        let mut assignments = Vec::new();
        while cursor != root {
            let r = counter::reservation(&self.store.repo, cursor)?;
            assignments.extend(r.assignments());
            cursor = r.parent;
        }
        Ok(assignments)
    }

    fn adopt_counter(&self, tip: gix::ObjectId, deadline: Instant) -> Result<()> {
        // update-ref performs its compare under Git's ref lock, including against foreign movers.
        let old = ref_target(&self.store.repo, SEQ_REF)?
            .map(|id| id.to_string())
            .unwrap_or_else(|| "0".repeat(tip.to_string().len()));
        self.git(
            &["update-ref".into(), SEQ_REF.into(), tip.to_string(), old],
            deadline,
        )?;
        Ok(())
    }

    fn fetch(&self, remote: &str, deadline: Instant) -> Result<Option<counter::Counter>> {
        let advertised = self.git(
            &[
                "ls-remote".into(),
                "--refs".into(),
                remote.into(),
                SEQ_REF.into(),
            ],
            deadline,
        )?;
        let mut args = vec![
            "fetch".into(),
            "--no-tags".into(),
            "--no-write-fetch-head".into(),
            "--no-auto-maintenance".into(),
            "--prune".into(),
            remote.into(),
            "+refs/tower/log/*:refs/tower/staging/log/*".into(),
        ];
        if !advertised.trim().is_empty() {
            args.push("+refs/tower/seq:refs/tower/staging/seq".into());
        } else if let Some(tip) = ref_target(&self.store.repo, "refs/tower/staging/seq")? {
            self.git(
                &[
                    "update-ref".into(),
                    "-d".into(),
                    "refs/tower/staging/seq".into(),
                    tip.to_string(),
                ],
                deadline,
            )?;
        }
        self.git(&args, deadline)?;
        ref_target(&self.store.repo, "refs/tower/staging/seq")?
            .map(|tip| counter::inspect(&self.store.repo, tip))
            .transpose()
    }

    fn staged_logs(&self) -> Result<Vec<(String, gix::ObjectId)>> {
        let mut refs = Vec::new();
        let platform = self.store.repo.references().map_err(Error::repo)?;
        for reference in platform
            .prefixed("refs/tower/staging/log/")
            .map_err(Error::repo)?
        {
            let reference = reference.map_err(Error::repo)?;
            let name = reference.name().as_bstr().to_string().replacen(
                "refs/tower/staging/log/",
                chain::LOG_PREFIX,
                1,
            );
            let id = reference
                .target()
                .try_id()
                .ok_or_else(|| refuse("staged log is symbolic"))?
                .to_owned();
            refs.push((name, id));
        }
        Ok(refs)
    }

    fn staged_events(&self) -> Result<Vec<Event>> {
        let mut events = Vec::new();
        for (name, tip) in self.staged_logs()? {
            let chain = super::walk(&self.store.repo, tip)?;
            for (next, event) in (1..).zip(chain.iter()) {
                if chain::log_ref(&event.author, &event.writer) != name
                    || event.id.writer != event.writer
                    || event.id.seq != next
                {
                    return Err(refuse(format!("invalid writer chain {name}")));
                }
            }
            events.extend(chain);
        }
        events.sort_by(|a, b| (a.time, &a.writer, a.id.seq).cmp(&(b.time, &b.writer, b.id.seq)));
        Ok(events)
    }

    fn promote_logs(&self, deadline: Instant) -> Result<()> {
        self.staged_events()?;
        for (name, tip) in self.staged_logs()? {
            let writer = name.rsplit('/').next().expect("writer");
            let _append = super::lock::acquire_until(&self.store.repo, writer, deadline)?;
            let current = ref_target(&self.store.repo, &name)?;
            if let Some(current) = current {
                if current == tip || self.log_ancestor(tip, current)? {
                    continue;
                }
                if !self.log_ancestor(current, tip)? {
                    return Err(refuse(format!(
                        "divergent writer chain {name}; both tips were preserved"
                    )));
                }
            }
            let old = current
                .map(|id| id.to_string())
                .unwrap_or_else(|| "0".repeat(tip.to_string().len()));
            self.git(&["update-ref".into(), name, tip.to_string(), old], deadline)?;
        }
        Ok(())
    }

    fn log_ancestor(&self, ancestor: gix::ObjectId, tip: gix::ObjectId) -> Result<bool> {
        let mut cursor = Some(tip);
        while let Some(id) = cursor {
            if id == ancestor {
                return Ok(true);
            }
            cursor = chain::decode(&self.store.repo, id)?.parent;
        }
        Ok(false)
    }

    fn git(&self, args: &[String], deadline: Instant) -> Result<String> {
        git(self.store.repo.common_dir(), args, deadline)
    }
}

fn refuse(detail: impl Into<String>) -> Error {
    Error::Sync {
        detail: detail.into(),
    }
}

/// File-backed output avoids pipe deadlocks and reader threads. An owned process group is reaped before return.
fn git(common: &std::path::Path, args: &[String], deadline: Instant) -> Result<String> {
    let stop = deadline
        .checked_sub(Duration::from_millis(50))
        .unwrap_or(deadline);
    if Instant::now() >= stop {
        return Err(Error::Deadline);
    }
    let mut stdout = tempfile::tempfile().map_err(Error::repo)?;
    let mut stderr = tempfile::tempfile().map_err(Error::repo)?;
    let mut command = Command::new("git");
    command
        .arg("--git-dir")
        .arg(common)
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "credential.interactive=false",
        ])
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .stdin(Stdio::null())
        .stdout(stdout.try_clone().map_err(Error::repo)?)
        .stderr(stderr.try_clone().map_err(Error::repo)?);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|err| Error::Transport {
        detail: err.to_string(),
    })?;
    #[cfg(windows)]
    let job = match windows_job::Job::assign(&child) {
        Ok(job) => job,
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Transport {
                detail: err.to_string(),
            });
        }
    };
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if Instant::now() < stop => std::thread::sleep(Duration::from_millis(5)),
            _ => break None,
        }
    };
    #[cfg(unix)]
    {
        // SAFETY: the child was spawned in a new process group with its own PID. Only that owned group is signaled.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
    }
    #[cfg(windows)]
    drop(job);
    if status.is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let status = status.ok_or(Error::Deadline)?;
    let mut output = String::new();
    if status.success() {
        stdout.rewind().map_err(Error::repo)?;
        stdout.read_to_string(&mut output).map_err(Error::repo)?;
        Ok(output)
    } else {
        stderr.rewind().map_err(Error::repo)?;
        stderr.read_to_string(&mut output).map_err(Error::repo)?;
        Err(Error::Transport {
            detail: output.trim().to_string(),
        })
    }
}

#[cfg(windows)]
mod windows_job {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    /// Closing the owned job synchronously terminates its transport tree without spawning a cleanup command.
    pub struct Job(HANDLE);

    impl Job {
        pub fn assign(child: &std::process::Child) -> std::io::Result<Self> {
            // SAFETY: handles are checked, the zero-initialized information has the documented layout, and its pointer lives through the call.
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err(std::io::Error::last_os_error());
                }
                let job = Self(handle);
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                    std::mem::size_of_val(&info) as u32,
                ) == 0
                    || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0
                {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(job)
            }
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: this struct owns the job handle and closes it exactly once.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn a_transport_deadline_cleans_up_its_owned_descendants() {
        let fixture = atc_testsupport::Repo::new();
        let store = Store::open(fixture.path()).unwrap();
        let marker = fixture.path().join("late-write");
        let alias = format!("alias.pause=!sleep 0.4; touch '{}'", marker.display());
        let start = Instant::now();
        let result = git(
            store.repo.common_dir(),
            &["-c".into(), alias, "pause".into()],
            start + Duration::from_millis(150),
        );
        assert!(matches!(result, Err(Error::Deadline)));
        assert!(start.elapsed() < Duration::from_millis(300));
        std::thread::sleep(Duration::from_millis(500));
        assert!(!marker.exists(), "a transport descendant survived return");
    }
}
