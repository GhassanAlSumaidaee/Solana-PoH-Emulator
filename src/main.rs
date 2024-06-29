use hex;
use serde::{Deserialize, Serialize};
use serde_json::to_vec;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread::JoinHandle;
use std::{thread, time};

#[derive(Serialize, Deserialize, Debug)]
//G.A: Defining a Transaction struct with more realistic fields for blockchain transactions :)
struct Transaction {
    sender: String,
    receiver: String,
    amount: u64,
    timestamp: u64,
}
// G.A: Poh struct where the core PoH operations are handeled.
struct Poh {
    hash_state: [u8; 32],
    num_hashes: u64,
    hashes_per_tick: u64,
    remaining_hashes: u64,
}
// G.A: Creating a new Poh instance with an initial seed and an optional number of hashes per tick.
impl Poh {
    fn new(seed: [u8; 32], hashes_per_tick: Option<u64>) -> Self {
        let hashes_per_tick = hashes_per_tick.unwrap_or(std::u64::MAX);
        assert!(hashes_per_tick > 1);
        Poh {
            hash_state: seed,
            num_hashes: 0,
            hashes_per_tick,
            remaining_hashes: hashes_per_tick,
        }
    }
    // G.A:  Generating a tick by hashing the current hash state.
    fn tick(&mut self) -> Option<Entry> {
        let mut hasher = Sha256::new();
        hasher.update(&self.hash_state);
        let result = hasher.finalize();
        self.hash_state.copy_from_slice(&result);
        self.num_hashes += 1;
        self.remaining_hashes -= 1;

        if self.hashes_per_tick != std::u64::MAX && self.remaining_hashes != 0 {
            return None;
        }

        let num_hashes = self.num_hashes;
        self.remaining_hashes = self.hashes_per_tick;
        self.num_hashes = 0;
        Some(Entry::new(num_hashes, self.hash_state))
    }
    //G.A:  Recording a transaction:  mixing in its hash and updating the current hash state.
    fn record(&mut self, mixin: &[u8; 32]) -> Option<Entry> {
        if self.remaining_hashes == 1 {
            return None; // Because caller needs to `tick()` first
        }

        let mut hasher = Sha256::new();
        hasher.update(&self.hash_state);
        hasher.update(mixin);
        let result = hasher.finalize();
        self.hash_state.copy_from_slice(&result);
        let num_hashes = self.num_hashes + 1;
        self.num_hashes = 0;
        self.remaining_hashes -= 1;

        Some(Entry::new(num_hashes, self.hash_state))
    }
}

#[derive(Debug)]
struct Entry {
    _num_hashes: u64,
    hash: [u8; 32],
}
// G.A: A new Entry with the given number of hashes and hash state.
impl Entry {
    fn new(num_hashes: u64, hash: [u8; 32]) -> Self {
        Entry {
            _num_hashes: num_hashes,
            hash,
        }
    }

    fn display(&self) {
        println!("Num hashes: {}, Hash: {:?}", self._num_hashes, self.hash);
    }
}

struct PohRecorder {
    poh: Poh,
    tick_height: u64,
    entry_queue: Vec<Entry>,
}
// G.A: Creating a new PohRecorder with the initial hash and optional number of hashes per tick :)
impl PohRecorder {
    fn new(initial_hash: [u8; 32], hashes_per_tick: Option<u64>) -> Self {
        PohRecorder {
            poh: Poh::new(initial_hash, hashes_per_tick),
            tick_height: 0,
            entry_queue: Vec::new(),
        }
    }
    //G.A:  Recording a transaction by converting it to a hash and adding it to the PoH.
    fn record(&mut self, transaction: &Transaction) {
        let tx_bytes = to_vec(&transaction).unwrap();
        let tx_hash = Sha256::digest(&tx_bytes).into();
        if let Some(entry) = self.poh.record(&tx_hash) {
            println!(
                "Recorded transaction from {} to {} of amount {}: {:?}",
                transaction.sender,
                transaction.receiver,
                transaction.amount,
                hex::encode(entry.hash)
            );
            entry.display();
            self.entry_queue.push(entry);
        }
    }
    // G.A: Generating a tick and updating the PoH state.
    fn tick(&mut self) {
        if let Some(entry) = self.poh.tick() {
            self.tick_height += 1;
            println!("Tick: {:?}", hex::encode(entry.hash));
            entry.display();
            self.entry_queue.push(entry);
        }
    }
}

struct PohService {
    poh_recorder: Arc<RwLock<PohRecorder>>,
    tick_duration: time::Duration,
    running: Arc<AtomicBool>,
    tick_handle: Option<JoinHandle<()>>,
    tx_handle: Option<JoinHandle<()>>,
}
//G.A:  Creating a new PohService with the given PoH recorder, tick duration, and transaction.
impl PohService {
    fn new(
        poh_recorder: Arc<RwLock<PohRecorder>>,
        tick_duration: time::Duration,
    ) -> (Self, Sender<Transaction>, Receiver<Transaction>) {
        let (tx_sender, tx_receiver) = channel();
        let service = PohService {
            poh_recorder,
            tick_duration,
            running: Arc::new(AtomicBool::new(false)),
            tick_handle: None,
            tx_handle: None,
        };
        (service, tx_sender, tx_receiver)
    }
    //G.A: We continuously produce ticks at the specified interval.
    fn tick_producer(
        poh_recorder: Arc<RwLock<PohRecorder>>,
        tick_duration: time::Duration,
        running: Arc<AtomicBool>,
    ) {
        while running.load(Ordering::Relaxed) {
            thread::sleep(tick_duration);
            poh_recorder.write().unwrap().tick();
        }
    }
    //G.A: We process incoming transactions and add them to the PoH.
    fn tx_processor(
        poh_recorder: Arc<RwLock<PohRecorder>>,
        running: Arc<AtomicBool>,
        tx_receiver: Receiver<Transaction>,
    ) {
        while running.load(Ordering::Relaxed) {
            if let Ok(transaction) = tx_receiver.recv() {
                poh_recorder.write().unwrap().record(&transaction);
            }
        }
    }
    //G.A:  Starting the PoH service with separate threads for ticking and transaction processing.
    fn start(&mut self, tx_receiver: Receiver<Transaction>) {
        let poh_recorder = Arc::clone(&self.poh_recorder);
        let tick_duration = self.tick_duration;
        let running = Arc::clone(&self.running);
        running.store(true, Ordering::Relaxed);
        self.tick_handle = Some(thread::spawn(move || {
            Self::tick_producer(poh_recorder, tick_duration, running);
        }));

        let poh_recorder = Arc::clone(&self.poh_recorder);
        let running = Arc::clone(&self.running);
        self.tx_handle = Some(thread::spawn(move || {
            Self::tx_processor(poh_recorder, running, tx_receiver);
        }));
    }
}

fn main() {
    //G.A: We initialize the PoH recorder with an initial hash and specify hashes per tick.
    let initial_hash = Sha256::digest(b"initial_seed").into();
    let poh_recorder = Arc::new(RwLock::new(PohRecorder::new(initial_hash, Some(10))));
    let (mut poh_service, tx_sender, tx_receiver) =
        PohService::new(Arc::clone(&poh_recorder), time::Duration::from_secs(1));
    // G.A: Starting the PoH service.
    poh_service.start(tx_receiver);

    // G.A: Not needed but I defined this set of transactions to be processed and for explanation and make it more realistic.
    let transactions1 = vec![
        Transaction {
            sender: "Ghassan".to_string(),
            receiver: "Sumaidaee".to_string(),
            amount: 50,
            timestamp: 1627489200,
        },
        Transaction {
            sender: "Drew".to_string(),
            receiver: "Alex".to_string(),
            amount: 25,
            timestamp: 1627489201,
        },
    ];
    // G.A: Sending the transactions to the PoH service.
    for tx in transactions1 {
        tx_sender.send(tx).unwrap();
    }
    // G.A: Sleeping for a while to simulate time passing.
    thread::sleep(time::Duration::from_secs(10));

    // G.A: Another set of transactions to be processed.
    let transactions2 = vec![
        Transaction {
            sender: "Maria".to_string(),
            receiver: "Jenny".to_string(),
            amount: 75,
            timestamp: 1627489202,
        },
        Transaction {
            sender: "Brennan".to_string(),
            receiver: "Rayn".to_string(),
            amount: 100,
            timestamp: 1627489203,
        },
    ];
    // G.A: Sending the new transactions to the PoH service.
    for tx in transactions2 {
        tx_sender.send(tx).unwrap();
    }

    // G.A: Keeping the main thread running indefinitely
    loop {
        thread::sleep(time::Duration::from_secs(60));
    }
}
