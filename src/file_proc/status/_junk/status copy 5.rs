#[derive(Hash, Eq, PartialEq)]
enum ProgressBarType {
    Scan,
    Hash,
    HashBar,
    CacheToDupe,
    Db,
}

fn init_pb() -> HashMap<ProgressBarType, ProgressBar> {
    let m = MultiProgress::new();

    let mut pb_map = HashMap::new();
    pb_map.insert(ProgressBarType::Scan, m.add(ProgressBar::new()));
    pb_map.insert(ProgressBarType::Hash, m.add(ProgressBar::new()));
    pb_map.insert(ProgressBarType::HashBar, m.add(ProgressBar::new()));
    pb_map.insert(ProgressBarType::CacheToDupe, m.add(ProgressBar::new()));
    pb_map.insert(ProgressBarType::Db, m.add(ProgressBar::new()));

    let bars = pb_map;

    bars
}

pub fn handle_status(rx: mpsc::Receiver<StatusMessage>, stats: Arc<Mutex<ProcessStats>>) {
    let bars = init_pb();

    let term = Term::stdout();
    let mut i = 0;
    let mut k = 0;

    for message in rx {
        i += 1;
        let mut _stats = stats.lock().unwrap();
        match message {
            StatusMessage::ScanBegin => {
                let message = format!("ScanBegin: {}", i);
                let b = bars.get(&ProgressBarType::Scan).unwrap();
                b.set_prefix("Scanning...");
                b.set_message(message);
            }
            StatusMessage::ScanAddRaw(_msg) => {
                let message = format!("ScanAddRaw: {}", i);
                let b = bars.get(&ProgressBarType::Scan).unwrap();
                b.set_message(message);
            }
            StatusMessage::ScanAddDupe(_msg) => {
                let message = format!("ScanAddDupe: {}", i);
                let b = bars.get(&ProgressBarType::Scan).unwrap();
                b.set_message(message);
            }
            StatusMessage::ScanEnd => {
                let message = format!("ScanEnd: {}", i);
                let b = bars.get(&ProgressBarType::Scan).unwrap();
                b.set_message(message.to_string());
                println!("DONE");
                b.finish_with_message(message.to_string());
            }
            StatusMessage::HashBegin => {
                let message = format!("HashBegin: {}", i);
                let b = bars.get(&ProgressBarType::Hash).unwrap();
                b.set_prefix("Hashing...");
                b.set_message(message);
                b.set_length(100);
            }
            StatusMessage::HashProc(_msg) => {
                let message = format!("HashProc: {}", i);
                let b = bars.get(&ProgressBarType::Hash).unwrap();
                b.set_message(message);
                if k < 100 {
                    k += 1;
                    b.set_position(k);
                } else {
                    b.finish_and_clear();
                }
            }
            StatusMessage::HashGenCacheFile(_msg) => {
                let message = format!("HashGenCacheFile: {}", i);
                let b = bars.get(&ProgressBarType::Hash).unwrap();
                b.set_message(message);
            }

            StatusMessage::HashEnd => {
                let message = format!("HashEnd: {}", i);
                let b = bars.get(&ProgressBarType::Hash).unwrap();
                b.set_message(message.to_string());
                b.finish_with_message(message.to_string());
            }
            StatusMessage::CacheToDupeBegin => {
                let message = format!("CacheToDupeBegin: {}", i);
                let b = bars.get(&ProgressBarType::CacheToDupe).unwrap();
                b.set_prefix("Cache to duping...");
                b.set_message(message);
            }
            StatusMessage::CacheToDupeProc(_msg) => {
                let message = format!("CacheToDupeProc: {}", i);
                let b = bars.get(&ProgressBarType::CacheToDupe).unwrap();
                b.set_message(message);
            }
            StatusMessage::CacheToDupeEnd => {
                let message = format!("CacheToDupeEnd: {}", i);
                let b = bars.get(&ProgressBarType::CacheToDupe).unwrap();
                b.set_message(message.to_string());
                b.finish_with_message(message.to_string());
            }
            StatusMessage::DbDupeFileInsertBegin => {
                let message = format!("DbDupeFileInsertBegin: {}", i);
                let b = bars.get(&ProgressBarType::Db).unwrap();
                b.set_prefix("DB Inserting...");
                b.set_message(message);
            }
            StatusMessage::DbDupeFileInsertProc(_msg) => {
                let message = format!("DbDupeFileInsertProc: {}", i);
                let b = bars.get(&ProgressBarType::Db).unwrap();
                b.set_message(message);
            }
            StatusMessage::DbDupeFileInsertEnd => {
                let message = format!("DbDupeFileInsertEnd: {}", i);
                let b = bars.get(&ProgressBarType::Db).unwrap();
                b.set_message(message.to_string());
                b.finish_with_message(message.to_string());
            }
        }
    }
}
