pub(crate) fn join_worker_in_background(
    label: &'static str,
    worker: Option<std::thread::JoinHandle<()>>,
) {
    let Some(worker) = worker else {
        return;
    };
    if worker.is_finished() {
        join_worker(label, worker);
        return;
    }
    if let Err(error) = std::thread::Builder::new()
        .name(format!("notslack-{label}-shutdown"))
        .spawn(move || join_worker(label, worker))
    {
        log::error!("failed to spawn {label} shutdown joiner: {error}");
    }
}

fn join_worker(label: &str, worker: std::thread::JoinHandle<()>) {
    if let Err(error) = worker.join() {
        if let Some(message) = error.downcast_ref::<String>() {
            log::error!("{label} thread panicked: {message}");
        } else if let Some(message) = error.downcast_ref::<&'static str>() {
            log::error!("{label} thread panicked: {message}");
        } else {
            log::error!("{label} thread panicked with unknown reason");
        }
    }
}
