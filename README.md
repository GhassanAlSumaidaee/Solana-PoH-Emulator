# Solana PoH Emulation

This project emulates the Proof of History (PoH) mechanism of Solana. The implementation involves creating a continuous PoH chain over time while receiving hashes to mix into the chain.

## Description

The project includes the following components:
- **Transaction**: Represents a transaction with sender, receiver, amount, and timestamp.
- **Poh**: Manages the PoH state and provides methods to generate ticks and record transactions.
- **Entry**: Represents an entry in the PoH chain, storing the number of hashes and the hash value.
- **PohRecorder**: Uses `Poh` to record transactions and generate ticks.
- **PohService**: Manages the background service for producing ticks and processing transactions.

## Usage

### Running the Project

1. Rust must be installed.
2. Clone the repository:
   ```sh
   git clone https://github.com/GhassanAlSumaidaee/Solana-PoH-Emulator.git

3. Navigate to the project directory:
```
cd Solana-PoH-Emulator
```

4. Build and run the project:

```
cargo run
```
## License
This project is highly private and created for Syndica for a specific purpose.
