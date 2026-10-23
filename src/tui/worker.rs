use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use crate::api::{self, NewTask, Patch};
use crate::error::Result;
use crate::mcp::Session;
use crate::model::{Task, TaskRef};

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
    Title {
        task_ref: TaskRef,
        title: String,
    },
    Note {
        task_ref: TaskRef,
        note: String,
    },
    Add {
        draft: NewTask,
    },
    Schedule {
        task_ref: TaskRef,
        patch: Patch,
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
        } => done(
            api::set_completed(session, &task_ref, completed).map(|_| ()),
            if completed { "done" } else { "reopened" },
        ),
        Request::Delete { task_ref } => {
            done(api::delete(session, &task_ref).map(|_| ()), "deleted")
        }
        Request::Title { task_ref, title } => done(edit_title(session, &task_ref, title), "saved"),
        Request::Note { task_ref, note } => done(edit_note(session, &task_ref, note), "note saved"),
        Request::Add { draft } => done(api::add(session, draft).map(|_| ()), "saved"),
        Request::Schedule { task_ref, patch } => {
            done(api::edit(session, &task_ref, patch).map(|_| ()), "saved")
        }
    }
}

fn fetch(session: &mut Session, key: &CacheKey) -> Result<Vec<Task>> {
    match key {
        CacheKey::Today => api::today(session),
        CacheKey::Inbox => api::inbox(session),
        CacheKey::Day(day) => api::day(session, day),
    }
}

fn edit_title(session: &mut Session, task_ref: &TaskRef, title: String) -> Result<()> {
    api::edit(
        session,
        task_ref,
        Patch {
            title: Some(title),
            note: None,
            day: None,
            start_time: None,
            duration: None,
            all_day: None,
            inbox: false,
            new_day: None,
        },
    )
    .map(|_| ())
}

fn edit_note(session: &mut Session, task_ref: &TaskRef, note: String) -> Result<()> {
    api::edit(
        session,
        task_ref,
        Patch {
            title: None,
            note: Some(note),
            day: None,
            start_time: None,
            duration: None,
            all_day: None,
            inbox: false,
            new_day: None,
        },
    )
    .map(|_| ())
}

fn done(result: Result<()>, message: &'static str) -> Reply {
    match result {
        Ok(()) => Reply::Saved { message },
        Err(err) => Reply::WriteFailed {
            message: err.to_string(),
        },
    }
}
