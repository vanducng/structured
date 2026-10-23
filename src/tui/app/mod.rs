mod edit;
mod keys;
mod schedule;

use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::Sender;

pub(in crate::tui) use keys::handle;

use crate::config::UiConfig;
use crate::model::{Task, TaskRef};

use super::worker::{CacheKey, Reply, Request};

pub(in crate::tui) enum Mode {
    Normal,
    Help,
    Confirm,
    Filter,
    Input { draft: String, prompt: Prompt },
}

pub(in crate::tui) enum Prompt {
    NewTitle,
    Title,
    Day,
    Time,
    Duration,
}

enum Undo {
    Complete {
        task_ref: TaskRef,
        was_completed: bool,
        was_at: Option<String>,
    },
    Delete {
        task: Task,
        index: usize,
        key: CacheKey,
    },
    Title {
        task_ref: TaskRef,
        previous: String,
    },
    Note {
        task_ref: TaskRef,
        previous: String,
    },
    Add {
        title: String,
    },
    Schedule {
        previous: Task,
        index: usize,
        key: CacheKey,
        removed: bool,
    },
}

pub(in crate::tui) struct App {
    pub(in crate::tui) inbox: bool,
    pub(in crate::tui) offset: i64,
    tasks: Vec<Task>,
    pub(in crate::tui) selected: usize,
    pub(in crate::tui) status: String,
    pub(in crate::tui) mode: Mode,
    g_pending: bool,
    pub(in crate::tui) query: String,
    filter_literal: bool,
    pub(in crate::tui) detail_scroll: u16,
    pub(in crate::tui) config: UiConfig,
    cache: HashMap<CacheKey, Vec<Task>>,
    want: Option<CacheKey>,
    prefetch: Option<CacheKey>,
    load_inflight: bool,
    epoch: u64,
    undos: VecDeque<Undo>,
    reload_when_idle: bool,
    checks: Vec<(TaskRef, bool)>,
}

impl App {
    pub(in crate::tui) fn new(config: UiConfig) -> Self {
        Self {
            inbox: false,
            offset: 0,
            tasks: Vec::new(),
            selected: 0,
            status: "loading".into(),
            mode: Mode::Normal,
            g_pending: false,
            query: String::new(),
            filter_literal: false,
            detail_scroll: 0,
            config,
            cache: HashMap::new(),
            want: None,
            prefetch: None,
            load_inflight: false,
            epoch: 0,
            undos: VecDeque::new(),
            reload_when_idle: false,
            checks: Vec::new(),
        }
    }

    pub(in crate::tui) fn visible(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|task| matches(task, &self.query))
            .collect()
    }

    pub(in crate::tui) fn selected_task(&self) -> Option<&Task> {
        self.visible().into_iter().nth(self.selected)
    }

    fn selected_index(&self) -> Option<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter(|(_, task)| matches(task, &self.query))
            .nth(self.selected)
            .map(|(index, _)| index)
    }

    pub(in crate::tui) fn heading(&self) -> String {
        if self.inbox {
            "Inbox".into()
        } else {
            day_heading(self.offset)
        }
    }

    fn cache_key(&self) -> CacheKey {
        if self.inbox {
            CacheKey::Inbox
        } else if self.offset == 0 {
            CacheKey::Today
        } else {
            CacheKey::Day(day_string(self.offset))
        }
    }

    fn clamp(&mut self) {
        let count = self.visible().len();
        if count == 0 {
            self.selected = 0;
        } else if self.selected >= count {
            self.selected = count - 1;
        }
    }

    pub(in crate::tui) fn reveal(&mut self) {
        let key = self.cache_key();
        self.selected = 0;
        self.detail_scroll = 0;
        if let Some(tasks) = self.cache.get(&key).cloned() {
            self.tasks = tasks;
            self.status = format!("{}  {} tasks", self.heading(), self.tasks.len());
        } else {
            self.tasks.clear();
            self.status = "loading".into();
        }
        self.want = Some(key);
    }

    pub(in crate::tui) fn arm_refresh(&mut self) {
        self.checks.clear();
        self.want = Some(self.cache_key());
    }

    pub(in crate::tui) fn should_poll_refresh(&self) -> bool {
        matches!(self.mode, Mode::Normal)
            && self.want.is_none()
            && !self.load_inflight
            && self.undos.is_empty()
    }

    pub(in crate::tui) fn pump(&mut self, tx: &Sender<Request>) {
        if self.load_inflight {
            return;
        }
        let (key, show) = if let Some(key) = self.want.take() {
            (key, true)
        } else if self.undos.is_empty() {
            let Some(key) = self.prefetch.take() else {
                return;
            };
            (key, false)
        } else {
            return;
        };
        self.load_inflight = true;
        let _ = tx.send(Request::Load {
            epoch: self.epoch,
            key,
            show,
        });
    }

    pub(in crate::tui) fn apply_reply(&mut self, reply: Reply) {
        match reply {
            Reply::Loaded {
                epoch,
                key,
                show,
                tasks,
            } => self.apply_load(epoch, key, tasks, show),
            Reply::LoadFailed { key, show, message } => self.fail_load(key, show, message),
            Reply::Saved { message } => self.finish_write(true, message.into()),
            Reply::WriteFailed { message } => self.finish_write(false, message),
        }
    }

    fn apply_load(&mut self, epoch: u64, key: CacheKey, tasks: Vec<Task>, show: bool) {
        self.load_inflight = false;
        if epoch != self.epoch {
            if key == self.cache_key() {
                if self.undos.is_empty() {
                    self.want = Some(key);
                } else {
                    self.reload_when_idle = true;
                }
            }
            return;
        }
        if show && key == self.cache_key() {
            self.replace_tasks(tasks);
            self.remember();
            if self.undos.is_empty() {
                self.status = format!("{}  {} tasks", self.heading(), self.tasks.len());
            }
        } else {
            self.cache.insert(key, tasks);
        }
        if show {
            self.consider_prefetch();
        }
    }

    fn fail_load(&mut self, key: CacheKey, show: bool, message: String) {
        self.load_inflight = false;
        if show && key == self.cache_key() {
            self.status = message;
        }
    }

    fn consider_prefetch(&mut self) {
        if self.want.is_some() || self.prefetch.is_some() || !self.undos.is_empty() {
            return;
        }
        let other = if self.inbox {
            CacheKey::Today
        } else {
            CacheKey::Inbox
        };
        if !self.cache.contains_key(&other) {
            self.prefetch = Some(other);
        }
    }

    fn replace_tasks(&mut self, tasks: Vec<Task>) {
        let selected = self.selected_task().and_then(|task| task.task_ref().ok());
        self.tasks = tasks;
        if let Some(task_ref) = selected {
            let query = self.query.clone();
            if let Some(pos) = self
                .tasks
                .iter()
                .filter(|task| matches(task, &query))
                .position(|task| task.task_ref().ok().as_ref() == Some(&task_ref))
            {
                self.selected = pos;
            }
        }
        self.clamp();
        self.apply_checks();
    }

    fn apply_checks(&mut self) {
        let checks = std::mem::take(&mut self.checks);
        let mut keep = Vec::new();
        for (task_ref, completed) in checks {
            let Some(task) = self
                .tasks
                .iter_mut()
                .find(|task| task.task_ref().ok().as_ref() == Some(&task_ref))
            else {
                keep.push((task_ref, completed));
                continue;
            };
            if task.done() == completed {
                continue;
            }
            task.is_completed = completed;
            task.completed_at = completed.then(|| "1".into());
            keep.push((task_ref, completed));
        }
        self.checks = keep;
    }

    fn remember(&mut self) {
        let key = self.cache_key();
        self.cache.insert(key, self.tasks.clone());
    }
}

pub(in crate::tui) fn day_string(offset: i64) -> String {
    (chrono::Local::now().date_naive() + chrono::Duration::days(offset))
        .format("%Y-%m-%d")
        .to_string()
}

pub(in crate::tui) fn day_heading(offset: i64) -> String {
    let date = chrono::Local::now().date_naive() + chrono::Duration::days(offset);
    let when = date.format("%a %d %b");
    match offset {
        0 => format!("Today  {when}"),
        1 => format!("Tomorrow  {when}"),
        -1 => format!("Yesterday  {when}"),
        n if n > 0 => format!("{when}  +{n}"),
        n => format!("{when}  {n}"),
    }
}

pub(in crate::tui) fn matches(task: &Task, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let query = query.to_lowercase();
    task.title.to_lowercase().contains(&query) || task.note.to_lowercase().contains(&query)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::UiConfig;

    fn task(title: &str, done: bool) -> Task {
        Task {
            id: Some(title.into()),
            recurring_id: None,
            title: title.into(),
            note: String::new(),
            day: None,
            start_time: None,
            duration: None,
            is_all_day: false,
            is_in_inbox: false,
            is_completed: done,
            is_recurring: false,
            completed_at: done.then(|| "1".into()),
            symbol: None,
            color: None,
            energy_level: None,
        }
    }

    #[test]
    fn cached_inbox_paints_without_waiting() {
        let mut app = App::new(UiConfig::default());
        app.tasks = vec![task("today", false)];
        app.cache
            .insert(CacheKey::Inbox, vec![task("later", false)]);
        app.inbox = true;
        app.reveal();
        assert_eq!(app.tasks[0].title, "later");
        assert_eq!(app.want, Some(CacheKey::Inbox));
        assert_ne!(app.status, "loading");
    }

    #[test]
    fn missing_cache_shows_loading() {
        let mut app = App::new(UiConfig::default());
        app.tasks = vec![task("today", false)];
        app.inbox = true;
        app.reveal();
        assert!(app.tasks.is_empty());
        assert_eq!(app.status, "loading");
    }

    #[test]
    fn tick_does_not_schedule_a_reload() {
        let mut app = App::new(UiConfig::default());
        app.tasks = vec![task("a", false)];
        assert!(app.toggle().is_some());
        app.finish_write(true, "done".into());
        assert!(app.tasks[0].done());
        assert!(app.want.is_none());
    }

    #[test]
    fn stale_reload_keeps_the_tick() {
        let mut app = App::new(UiConfig::default());
        app.tasks = vec![task("a", false)];
        assert!(app.toggle().is_some());
        app.apply_load(0, CacheKey::Today, vec![task("a", false)], true);
        assert!(app.tasks[0].done());
        app.finish_write(true, "done".into());
        assert_eq!(app.want, Some(CacheKey::Today));
    }

    #[test]
    fn uncheck_stays_blank_when_the_server_still_has_a_stamp() {
        let mut app = App::new(UiConfig::default());
        let mut stamped = task("a", false);
        stamped.completed_at = Some("2026-10-23T01:00:00Z".into());
        app.tasks = vec![stamped.clone()];
        assert!(app.tasks[0].done());
        assert!(app.toggle().is_some());
        assert!(!app.tasks[0].done());
        let epoch = app.epoch;
        app.apply_load(epoch, CacheKey::Today, vec![stamped], true);
        assert!(!app.tasks[0].done());
        assert!(app.tasks[0].completed_at.is_none());
        assert!(app.toggle().is_some());
        assert!(app.tasks[0].done());
    }

    #[test]
    fn headings_and_filter() {
        assert!(day_heading(0).starts_with("Today"));
        assert!(day_heading(1).starts_with("Tomorrow"));
        assert!(matches(&task("Write notes", false), "note"));
        assert!(!matches(&task("Write notes", false), "missing"));
    }
}
