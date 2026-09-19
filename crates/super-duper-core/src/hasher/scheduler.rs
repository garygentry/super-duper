use crate::platform::{StorageDevice, StorageMediaClass};
use std::collections::{HashMap, VecDeque};
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

pub(crate) const ROTATIONAL_READERS: usize = 1;
pub(crate) const UNKNOWN_DEVICE_READERS: usize = 1;
pub(crate) const SOLID_STATE_READERS: usize = 4;

#[derive(Debug)]
pub(crate) struct ScheduledRead<T> {
    pub device: StorageDevice,
    pub value: T,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DeviceReadPolicy {
    global_readers: usize,
    solid_state_readers: usize,
}

impl Default for DeviceReadPolicy {
    fn default() -> Self {
        Self {
            global_readers: std::thread::available_parallelism()
                .map(usize::from)
                .unwrap_or(1)
                .max(1),
            solid_state_readers: SOLID_STATE_READERS,
        }
    }
}

impl DeviceReadPolicy {
    #[cfg(test)]
    pub(crate) fn for_test(global_readers: usize, solid_state_readers: usize) -> Self {
        Self {
            global_readers: global_readers.max(1),
            solid_state_readers: solid_state_readers.max(1),
        }
    }

    fn readers_for(self, media: StorageMediaClass) -> usize {
        match media {
            StorageMediaClass::Rotational => ROTATIONAL_READERS,
            StorageMediaClass::SolidState => self.solid_state_readers,
            StorageMediaClass::Unknown => UNKNOWN_DEVICE_READERS,
        }
        .min(self.global_readers)
        .max(1)
    }
}

struct DeviceQueue<T> {
    pending: VecDeque<(usize, T)>,
    active: usize,
    limit: usize,
}

struct SchedulerState<T, R> {
    queues: Vec<DeviceQueue<T>>,
    next_queue: usize,
    active: usize,
    stopped: bool,
    error: Option<io::Error>,
    results: Vec<Option<R>>,
}

pub(crate) fn execute_device_reads<T, R, F>(
    tasks: Vec<ScheduledRead<T>>,
    cancel: &AtomicBool,
    policy: DeviceReadPolicy,
    work: F,
) -> io::Result<Vec<R>>
where
    T: Send,
    R: Send,
    F: Fn(T) -> io::Result<R> + Sync,
{
    if tasks.is_empty() {
        return Ok(Vec::new());
    }

    let task_count = tasks.len();
    let mut queue_indexes = HashMap::<String, usize>::new();
    let mut queues = Vec::<DeviceQueue<T>>::new();
    for (task_index, task) in tasks.into_iter().enumerate() {
        let requested_limit = policy.readers_for(task.device.media);
        let queue_index = match queue_indexes.get(&task.device.key) {
            Some(index) => {
                // Conflicting media observations for one physical key fail conservatively.
                queues[*index].limit = queues[*index].limit.min(requested_limit);
                *index
            }
            None => {
                let index = queues.len();
                queue_indexes.insert(task.device.key, index);
                queues.push(DeviceQueue {
                    pending: VecDeque::new(),
                    active: 0,
                    limit: requested_limit,
                });
                index
            }
        };
        queues[queue_index]
            .pending
            .push_back((task_index, task.value));
    }

    let shared = (
        Mutex::new(SchedulerState {
            queues,
            next_queue: 0,
            active: 0,
            stopped: false,
            error: None,
            results: std::iter::repeat_with(|| None).take(task_count).collect(),
        }),
        Condvar::new(),
    );
    let worker_count = policy.global_readers.min(task_count).max(1);

    std::thread::scope(|scope| {
        for _ in 0..worker_count {
            let shared = &shared;
            let work = &work;
            scope.spawn(move || {
                loop {
                    let task = {
                        let (lock, ready) = shared;
                        let mut state =
                            lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                        loop {
                            if cancel.load(Ordering::Acquire) && !state.stopped {
                                state.stopped = true;
                                state.error = Some(io::Error::new(
                                    io::ErrorKind::Interrupted,
                                    "hash scheduling cancelled",
                                ));
                                for queue in &mut state.queues {
                                    queue.pending.clear();
                                }
                            }
                            if state.stopped {
                                if state.active == 0 {
                                    return;
                                }
                                state = ready
                                    .wait(state)
                                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                                continue;
                            }

                            let queue_count = state.queues.len();
                            let mut selected = None;
                            for offset in 0..queue_count {
                                let index = (state.next_queue + offset) % queue_count;
                                let queue = &state.queues[index];
                                if queue.active < queue.limit && !queue.pending.is_empty() {
                                    selected = Some(index);
                                    break;
                                }
                            }
                            if let Some(index) = selected {
                                state.next_queue = (index + 1) % queue_count;
                                let task = state.queues[index]
                                    .pending
                                    .pop_front()
                                    .expect("selected queue must contain a task");
                                state.queues[index].active += 1;
                                state.active += 1;
                                break (index, task);
                            }
                            if state.active == 0 {
                                return;
                            }
                            state = ready
                                .wait(state)
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                        }
                    };

                    let (queue_index, (task_index, value)) = task;
                    let result = if cancel.load(Ordering::Acquire) {
                        Err(io::Error::new(
                            io::ErrorKind::Interrupted,
                            "hash scheduling cancelled",
                        ))
                    } else {
                        work(value)
                    };

                    let (lock, ready) = shared;
                    let mut state = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    state.queues[queue_index].active -= 1;
                    state.active -= 1;
                    match result {
                        Ok(value) if !state.stopped => state.results[task_index] = Some(value),
                        Ok(_) => {}
                        Err(error) if !state.stopped => {
                            state.stopped = true;
                            state.error = Some(error);
                            for queue in &mut state.queues {
                                queue.pending.clear();
                            }
                        }
                        Err(_) => {}
                    }
                    ready.notify_all();
                }
            });
        }
    });

    let mut state = shared
        .0
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(error) = state.error.take() {
        return Err(error);
    }
    state
        .results
        .into_iter()
        .map(|result| result.ok_or_else(|| io::Error::other("device scheduler omitted a result")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Barrier, mpsc};
    use std::time::Duration;

    fn task(device: &str, media: StorageMediaClass, value: usize) -> ScheduledRead<usize> {
        ScheduledRead {
            device: StorageDevice {
                key: device.to_owned(),
                media,
            },
            value,
        }
    }

    fn observe_max(maximum: &AtomicUsize, value: usize) {
        let mut current = maximum.load(Ordering::Relaxed);
        while value > current {
            match maximum.compare_exchange_weak(
                current,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }

    #[test]
    fn rotational_and_unknown_devices_are_serialized() {
        for media in [StorageMediaClass::Rotational, StorageMediaClass::Unknown] {
            let active = AtomicUsize::new(0);
            let maximum = AtomicUsize::new(0);
            let values = execute_device_reads(
                (0..12).map(|value| task("same", media, value)).collect(),
                &AtomicBool::new(false),
                DeviceReadPolicy::for_test(8, 4),
                |value| {
                    let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                    observe_max(&maximum, now);
                    std::thread::sleep(Duration::from_millis(1));
                    active.fetch_sub(1, Ordering::SeqCst);
                    Ok(value)
                },
            )
            .unwrap();
            assert_eq!(values, (0..12).collect::<Vec<_>>());
            assert_eq!(maximum.load(Ordering::SeqCst), 1);
        }
    }

    #[test]
    fn solid_state_device_uses_its_bounded_reader_ceiling() {
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        let first_readers = Barrier::new(4);
        execute_device_reads(
            (0..4)
                .map(|value| task("ssd", StorageMediaClass::SolidState, value))
                .collect(),
            &AtomicBool::new(false),
            DeviceReadPolicy::for_test(8, 4),
            |value| {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                observe_max(&maximum, now);
                first_readers.wait();
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(value)
            },
        )
        .unwrap();
        assert_eq!(maximum.load(Ordering::SeqCst), 4);
    }

    #[test]
    fn separate_rotational_devices_progress_independently() {
        let barrier = Arc::new(Barrier::new(3));
        let worker_barrier = barrier.clone();
        let join = std::thread::spawn(move || {
            execute_device_reads(
                vec![
                    task("disk-a", StorageMediaClass::Rotational, 1),
                    task("disk-b", StorageMediaClass::Rotational, 2),
                ],
                &AtomicBool::new(false),
                DeviceReadPolicy::for_test(2, 2),
                |value| {
                    worker_barrier.wait();
                    Ok(value)
                },
            )
        });
        barrier.wait();
        assert_eq!(join.join().unwrap().unwrap(), vec![1, 2]);
    }

    #[test]
    fn cancellation_prevents_queued_content_work_from_starting() {
        let cancel = Arc::new(AtomicBool::new(false));
        let opens = Arc::new(AtomicUsize::new(0));
        let (started_tx, started_rx) = mpsc::channel();
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let worker_cancel = cancel.clone();
        let worker_opens = opens.clone();
        let worker_release = release.clone();
        let join = std::thread::spawn(move || {
            execute_device_reads(
                (0..8)
                    .map(|value| task("hdd", StorageMediaClass::Rotational, value))
                    .collect(),
                &worker_cancel,
                DeviceReadPolicy::for_test(8, 4),
                |value| {
                    worker_opens.fetch_add(1, Ordering::SeqCst);
                    if value == 0 {
                        started_tx.send(()).unwrap();
                        let (lock, ready) = &*worker_release;
                        let mut released =
                            lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                        while !*released {
                            released = ready
                                .wait(released)
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                        }
                    }
                    Ok(value)
                },
            )
        });
        started_rx.recv().unwrap();
        cancel.store(true, Ordering::Release);
        let (lock, ready) = &*release;
        *lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        ready.notify_all();
        let error = join.join().unwrap().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(opens.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn conflicting_media_for_one_key_uses_the_more_conservative_limit() {
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        execute_device_reads(
            vec![
                task("disk", StorageMediaClass::SolidState, 1),
                task("disk", StorageMediaClass::Rotational, 2),
                task("disk", StorageMediaClass::SolidState, 3),
            ],
            &AtomicBool::new(false),
            DeviceReadPolicy::for_test(4, 4),
            |value| {
                let now = active.fetch_add(1, Ordering::SeqCst) + 1;
                observe_max(&maximum, now);
                std::thread::sleep(Duration::from_millis(1));
                active.fetch_sub(1, Ordering::SeqCst);
                Ok(value)
            },
        )
        .unwrap();
        assert_eq!(maximum.load(Ordering::SeqCst), 1);
    }
}
