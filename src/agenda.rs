use chrono::NaiveDate;

use crate::clock;
use crate::model::Task;

const GAP_MINUTES: i64 = 30;
/// The rolling view spans this many hours on each side of now, so an evening-to-morning
/// working session that crosses midnight stays in one list.
pub const WINDOW_HOURS: f64 = 12.0;

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Section(String),
    Task {
        index: usize,
        overlap: bool,
        late: bool,
    },
    Gap(i64),
    Now(f64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Layout {
    Inbox,
    Day { now_line: bool },
    Window,
}

/// The wall clock as hours since today's midnight, so tasks on nearby days compare on one axis.
#[derive(Clone, Copy, Debug)]
pub struct Clock {
    pub today: NaiveDate,
    pub hours: f64,
}

impl Clock {
    pub fn now() -> Self {
        Self {
            today: clock::today(),
            hours: clock::now_hours(),
        }
    }

    fn day_offset(&self, task: &Task) -> Option<i64> {
        let day = clock::parse_stamp(task.day.as_deref()?)?;
        Some((day - self.today).num_days())
    }

    fn span(&self, task: &Task) -> Option<(f64, f64)> {
        let start = self.day_offset(task)? as f64 * 24.0 + task.timed()?;
        Some((start, start + task.minutes() as f64 / 60.0))
    }

    pub fn late(&self, task: &Task) -> bool {
        if task.done() {
            return false;
        }
        match (self.span(task), self.day_offset(task)) {
            (Some((_, end)), _) => end < self.hours,
            (None, Some(offset)) => offset < 0,
            _ => false,
        }
    }

    /// Timed work that touches the window, today's all-day tasks, everything unfinished from
    /// yesterday, and tomorrow's all-day tasks once the window reaches past midnight.
    pub fn in_window(&self, task: &Task) -> bool {
        let Some(offset) = self
            .day_offset(task)
            .filter(|offset| (-1..=1).contains(offset))
        else {
            return false;
        };
        if offset == -1 && !task.done() {
            return true;
        }
        match self.span(task) {
            Some((start, end)) => {
                end >= self.hours - WINDOW_HOURS && start <= self.hours + WINDOW_HOURS
            }
            None => match offset {
                0 => true,
                -1 => false,
                _ => self.hours + WINDOW_HOURS >= 24.0,
            },
        }
    }

    pub fn window_days(&self) -> [String; 3] {
        [-1, 0, 1].map(|offset| clock::stamp(self.today + chrono::Duration::days(offset)))
    }
}

/// Days read top-down like the app: by day, all-day first, then start time. Inbox puts open work first.
pub fn sort(tasks: &mut [Task], inbox: bool) {
    if inbox {
        tasks.sort_by_key(Task::done);
        return;
    }
    let key = |task: &Task| {
        (
            task.day.clone().unwrap_or_default(),
            task.timed().unwrap_or(f64::NEG_INFINITY),
        )
    };
    tasks.sort_by(|a, b| {
        let (a, b) = (key(a), key(b));
        a.0.cmp(&b.0).then(a.1.total_cmp(&b.1))
    });
}

pub fn rows(tasks: &[&Task], layout: Layout, clock: &Clock) -> Vec<Row> {
    match layout {
        Layout::Inbox => inbox_rows(tasks),
        Layout::Day { now_line } => {
            let mut rows = Vec::with_capacity(tasks.len() + 4);
            if tasks.iter().any(|task| task.timed().is_none()) {
                rows.push(Row::Section("All day".into()));
            }
            let now = now_line.then_some(clock.hours);
            day_rows(&mut rows, tasks, (0..tasks.len()).collect(), clock, now);
            rows
        }
        Layout::Window => window_rows(tasks, clock),
    }
}

/// One section per day, so a session that starts yesterday evening reads straight into today.
fn window_rows(tasks: &[&Task], clock: &Clock) -> Vec<Row> {
    let mut rows = Vec::with_capacity(tasks.len() + 8);
    let today = clock::stamp(clock.today);
    for day in clock.window_days() {
        let members: Vec<usize> = (0..tasks.len())
            .filter(|&i| tasks[i].day.as_deref() == Some(day.as_str()))
            .collect();
        if members.is_empty() && day != today {
            continue;
        }
        let date = clock::parse_stamp(&day).unwrap_or(clock.today);
        let word = clock::relative(date)
            .map(|word| capitalize(&word))
            .unwrap_or_default();
        rows.push(Row::Section(format!(
            "{word} · {}",
            date.format("%a %-d %b")
        )));
        let now = (day == today).then_some(clock.hours);
        day_rows(&mut rows, tasks, members, clock, now);
    }
    rows
}

fn capitalize(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// One day's rows: all-day tasks, then timed tasks with free gaps, overlaps, and the now-line.
fn day_rows(
    rows: &mut Vec<Row>,
    tasks: &[&Task],
    members: Vec<usize>,
    clock: &Clock,
    now: Option<f64>,
) {
    let mut busy_until: Option<f64> = None;
    let mut now = now;
    for index in members {
        let task = tasks[index];
        let late = clock.late(task);
        let Some(start) = task.timed() else {
            rows.push(Row::Task {
                index,
                overlap: false,
                late,
            });
            continue;
        };
        let mut overlap = false;
        if let Some(until) = busy_until {
            let free = ((start - until) * 60.0).round() as i64;
            overlap = free < 0;
            if free >= GAP_MINUTES {
                rows.push(Row::Gap(free));
            }
        }
        if let Some(at) = now.filter(|at| *at < start) {
            rows.push(Row::Now(at));
            now = None;
        }
        rows.push(Row::Task {
            index,
            overlap,
            late,
        });
        let end = task.end().unwrap_or(start);
        busy_until = Some(busy_until.map_or(end, |until: f64| until.max(end)));
    }
    if let Some(at) = now {
        rows.push(Row::Now(at));
    }
}

fn inbox_rows(tasks: &[&Task]) -> Vec<Row> {
    let done = tasks.iter().filter(|task| task.done()).count();
    let mut rows = Vec::with_capacity(tasks.len() + 1);
    let mut marked = false;
    for (index, task) in tasks.iter().enumerate() {
        if task.done() && !marked {
            rows.push(Row::Section(format!("Done · {done}")));
            marked = true;
        }
        rows.push(Row::Task {
            index,
            overlap: false,
            late: false,
        });
    }
    rows
}

pub fn planned_minutes(tasks: &[&Task]) -> i64 {
    tasks
        .iter()
        .filter(|task| task.timed().is_some())
        .map(|task| task.minutes())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(day: &str, title: &str, start: Option<f64>, minutes: i64) -> Task {
        Task {
            title: title.into(),
            day: Some(day.into()),
            start_time: start,
            duration: Some(minutes),
            is_all_day: start.is_none(),
            ..Task::default()
        }
    }

    fn clock(hours: f64) -> Clock {
        Clock {
            today: NaiveDate::from_ymd_opt(2026, 10, 24).unwrap(),
            hours,
        }
    }

    #[test]
    fn day_rows_show_all_day_first_then_gaps_overlaps_late_and_now() {
        let day = "2026-10-24";
        let mut tasks = vec![
            at(day, "review", Some(9.0), 30),
            at(day, "run", None, 0),
            at(day, "sync", Some(8.5), 45),
            at(day, "deep work", Some(13.0), 210),
        ];
        sort(&mut tasks, false);
        let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["run", "sync", "review", "deep work"]);
        let refs: Vec<&Task> = tasks.iter().collect();
        let rows = rows(&refs, Layout::Day { now_line: true }, &clock(10.0));
        assert_eq!(
            rows,
            vec![
                Row::Section("All day".into()),
                Row::Task {
                    index: 0,
                    overlap: false,
                    late: false
                },
                Row::Task {
                    index: 1,
                    overlap: false,
                    late: true
                },
                Row::Task {
                    index: 2,
                    overlap: true,
                    late: true
                },
                Row::Gap(210),
                Row::Now(10.0),
                Row::Task {
                    index: 3,
                    overlap: false,
                    late: false
                },
            ]
        );
        assert_eq!(planned_minutes(&refs), 285);
    }

    #[test]
    fn window_spans_midnight_and_carries_unfinished_work() {
        let now = clock(2.0);
        let late_evening = at("2026-10-23", "standup ET", Some(21.0), 30);
        let open_this_morning = at("2026-10-23", "breakfast", Some(7.0), 30);
        let mut long_ago = open_this_morning.clone();
        long_ago.completed_at = Some("2026-10-23T07:30:00Z".into());
        let open_yesterday = at("2026-10-23", "write report", None, 0);
        let mut done_yesterday = at("2026-10-23", "laundry", None, 0);
        done_yesterday.completed_at = Some("2026-10-23T09:00:00Z".into());
        let tonight = at("2026-10-24", "sync", Some(13.5), 30);
        let too_late = at("2026-10-24", "dinner", Some(19.0), 60);
        let tomorrow_all_day = at("2026-10-25", "trip", None, 0);
        assert!(now.in_window(&late_evening));
        assert!(!now.in_window(&long_ago));
        assert!(now.in_window(&open_this_morning));
        assert!(now.late(&open_this_morning));
        assert!(now.in_window(&open_yesterday));
        assert!(!now.in_window(&done_yesterday));
        assert!(now.in_window(&tonight));
        assert!(!now.in_window(&too_late));
        assert!(!now.in_window(&tomorrow_all_day));
        assert!(clock(14.0).in_window(&tomorrow_all_day));
        assert!(now.late(&late_evening));
        assert!(now.late(&open_yesterday));
        assert!(!now.late(&tonight));
    }

    #[test]
    fn window_rows_group_by_day_with_the_now_line_in_today() {
        let mut tasks = vec![
            at("2026-10-24", "sync", Some(9.0), 30),
            at("2026-10-23", "standup ET", Some(21.0), 30),
        ];
        sort(&mut tasks, false);
        let refs: Vec<&Task> = tasks.iter().collect();
        let rows = rows(&refs, Layout::Window, &clock(2.0));
        assert!(matches!(&rows[0], Row::Section(name) if name.contains("23 Oct")));
        assert_eq!(
            rows[1],
            Row::Task {
                index: 0,
                overlap: false,
                late: true
            }
        );
        assert!(matches!(&rows[2], Row::Section(name) if name.contains("24 Oct")));
        assert_eq!(rows[3], Row::Now(2.0));
        assert_eq!(
            rows[4],
            Row::Task {
                index: 1,
                overlap: false,
                late: false
            }
        );
    }

    #[test]
    fn inbox_puts_done_last_under_a_section() {
        let mut done = at("2026-10-24", "old", None, 0);
        done.day = None;
        done.completed_at = Some("2026-10-01T00:00:00Z".into());
        let mut open = done.clone();
        open.title = "new".into();
        open.completed_at = None;
        let mut tasks = vec![done, open];
        sort(&mut tasks, true);
        let refs: Vec<&Task> = tasks.iter().collect();
        let rows = rows(&refs, Layout::Inbox, &clock(10.0));
        assert_eq!(
            rows[0],
            Row::Task {
                index: 0,
                overlap: false,
                late: false
            }
        );
        assert_eq!(rows[1], Row::Section("Done · 1".into()));
    }
}
