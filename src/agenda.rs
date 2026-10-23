use crate::model::Task;

const GAP_MINUTES: i64 = 30;

#[derive(Clone, Debug, PartialEq)]
pub enum Row {
    Section(String),
    Task { index: usize, overlap: bool },
    Gap(i64),
    Now(f64),
}

/// Day views read top-down like the app: all-day first, then by start time. Inbox puts open work first.
pub fn sort(tasks: &mut [Task], inbox: bool) {
    if inbox {
        tasks.sort_by_key(Task::done);
    } else {
        tasks.sort_by(|a, b| {
            let a = a.timed().unwrap_or(f64::NEG_INFINITY);
            let b = b.timed().unwrap_or(f64::NEG_INFINITY);
            a.total_cmp(&b)
        });
    }
}

pub fn rows(tasks: &[&Task], inbox: bool, now: Option<f64>) -> Vec<Row> {
    if inbox {
        return inbox_rows(tasks);
    }
    let mut rows = Vec::with_capacity(tasks.len() + 4);
    if tasks.iter().any(|task| task.timed().is_none()) {
        rows.push(Row::Section("All day".into()));
    }
    let mut busy_until: Option<f64> = None;
    let mut now = now.filter(|_| tasks.iter().any(|task| task.timed().is_some()));
    for (index, task) in tasks.iter().enumerate() {
        let Some(start) = task.timed() else {
            rows.push(Row::Task {
                index,
                overlap: false,
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
        rows.push(Row::Task { index, overlap });
        let end = task.end().unwrap_or(start);
        busy_until = Some(busy_until.map_or(end, |until: f64| until.max(end)));
    }
    if let Some(at) = now {
        rows.push(Row::Now(at));
    }
    rows
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

    fn timed(title: &str, start: f64, minutes: i64) -> Task {
        Task {
            title: title.into(),
            day: Some("2026-10-23".into()),
            start_time: Some(start),
            duration: Some(minutes),
            ..Task::default()
        }
    }

    fn all_day(title: &str) -> Task {
        Task {
            title: title.into(),
            day: Some("2026-10-23".into()),
            is_all_day: true,
            ..Task::default()
        }
    }

    #[test]
    fn day_rows_show_all_day_first_then_gaps_overlaps_and_now() {
        let mut tasks = vec![
            timed("review", 9.0, 30),
            all_day("run"),
            timed("sync", 8.5, 45),
            timed("deep work", 13.0, 210),
        ];
        sort(&mut tasks, false);
        let titles: Vec<&str> = tasks.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["run", "sync", "review", "deep work"]);
        let refs: Vec<&Task> = tasks.iter().collect();
        let rows = rows(&refs, false, Some(10.0));
        assert_eq!(
            rows,
            vec![
                Row::Section("All day".into()),
                Row::Task {
                    index: 0,
                    overlap: false
                },
                Row::Task {
                    index: 1,
                    overlap: false
                },
                Row::Task {
                    index: 2,
                    overlap: true
                },
                Row::Gap(210),
                Row::Now(10.0),
                Row::Task {
                    index: 3,
                    overlap: false
                },
            ]
        );
        assert_eq!(planned_minutes(&refs), 285);
    }

    #[test]
    fn inbox_puts_done_last_under_a_section() {
        let mut done = all_day("old");
        done.completed_at = Some("2026-10-01T00:00:00Z".into());
        let mut tasks = vec![done, all_day("new")];
        sort(&mut tasks, true);
        let refs: Vec<&Task> = tasks.iter().collect();
        let rows = rows(&refs, true, None);
        assert_eq!(
            rows[0],
            Row::Task {
                index: 0,
                overlap: false
            }
        );
        assert_eq!(rows[1], Row::Section("Done · 1".into()));
        assert_eq!(refs[0].title, "new");
    }
}
