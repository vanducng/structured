use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::api::{self, NewTask, Patch, Scope};
use crate::error::Result;
use crate::mcp::Session;
use crate::model::{Subtask, Task, TaskRef};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::tui) enum CacheKey {
    Today,
    Inbox,
    Day(String),
}

pub(in crate::tui) enum Request {
    Load {
        epoch: u64,
        key: CacheKey,
        show: bool,
    },
    Complete {
        task_ref: TaskRef,
        completed: bool,
    },
    Delete {
        task_ref: TaskRef,
    },
    Edit {
        task_ref: TaskRef,
        patch: Patch,
        scope: Option<Scope>,
    },
    Add {
        draft: NewTask,
    },
    Subtasks {
        task_ref: TaskRef,
        subtasks: Vec<Subtask>,
        structural: bool,
    },
}

pub(in crate::tui) enum Reply {
    Loaded {
        epoch: u64,
        key: CacheKey,
        show: bool,
        tasks: Vec<Task>,
    },
    LoadFailed {
        key: CacheKey,
        show: bool,
        message: String,
    },
    Saved {
        message: &'static str,
        task: Option<Box<Task>>,
    },
    WriteFailed {
        message: String,
    },
}

pub(in crate::tui) fn spawn() -> Result<(Sender<Request>, Receiver<Reply>)> {
    let session = Session::connect()?;
    let (request_tx, request_rx) = mpsc::channel();
    let (reply_tx, reply_rx) = mpsc::channel();
    thread::Builder::new()
        .name("structured-io".into())
        .spawn(move || serve(session, request_rx, reply_tx))?;
    Ok((request_tx, reply_rx))
}

fn serve(mut session: Session, requests: Receiver<Request>, replies: Sender<Reply>) {
    while let Ok(request) = requests.recv() {
        if replies.send(dispatch(&mut session, request)).is_err() {
            break;
        }
    }
}

fn dispatch(session: &mut Session, request: Request) -> Reply {
    match request {
        Request::Load { epoch, key, show } => match fetch(session, &key) {
            Ok(tasks) => Reply::Loaded {
                epoch,
                key,
                show,
                tasks,
            },
            Err(err) => Reply::LoadFailed {
                key,
                show,
                message: err.to_string(),
            },
        },
        Request::Complete {
            task_ref,
            completed,
        } => saved(
            api::set_completed(session, &task_ref, completed),
            if completed { "done" } else { "reopened" },
        ),
        Request::Delete { task_ref } => saved(api::delete(session, &task_ref), "deleted"),
        Request::Edit {
            task_ref,
            patch,
            scope,
        } => saved(api::edit(session, &task_ref, patch, scope), "saved"),
        Request::Add { draft } => saved(api::add(session, draft), "added"),
        Request::Subtasks {
            task_ref,
            subtasks,
            structural,
        } => saved(
            api::set_subtasks(session, &task_ref, &subtasks, structural),
            "subtasks saved",
        ),
    }
}

fn fetch(session: &mut Session, key: &CacheKey) -> Result<Vec<Task>> {
    match key {
        CacheKey::Today => api::today(session),
        CacheKey::Inbox => api::inbox(session),
        CacheKey::Day(day) => api::day(session, day),
    }
}

fn saved(result: Result<serde_json::Value>, message: &'static str) -> Reply {
    match result {
        Ok(value) => Reply::Saved {
            message,
            task: api::task_in(&value).map(Box::new),
        },
        Err(err) => Reply::WriteFailed {
            message: err.to_string(),
        },
    }
}
