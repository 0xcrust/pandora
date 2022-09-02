# Pandora
Pandora: Decentralized fundraising platform.

## Description
Pandora is a smart contract that replicates the idea of a traditional fundraising platform with which users 
can source for funds from benefactors, except it's fully decentralized! It implements the concept of rounds
(a single fundraising campaign might have multiple rounds) and allows for staking of a native token. Stakers 
get benefits such as being able to flag a campaign as fraudulent and taking part in voting to decide whether 
or not a campaign may begin the next round. Donators to a round of a campaign also get to participate in the
vote that decides whether or not that campaign may progress to the next round based on its perceived validity.

Pandora lives on the Solana devnet at address: **DLkygNkiyVjJ4hu2fVV7M1fjX8DKdXbB3TgFmfwwKfqr**


## Requirements
- [Rust](https://www.rust-lang.org/tools/install)
- [Solana](https://docs.solana.com/cli/install-solana-cli-tools)
- [Yarn](https://yarnpkg.com/getting-started/install)
- [Anchor](https://book.anchor-lang.com/getting_started/installation.html)

View the full steps [here.](https://book.anchor-lang.com/getting_started/installation.html)

## Build and Testing
Deploy the contract to the devnet by following these steps on your cli:

#### Generate wallet
- Run ` solana-keygen new ` to create a wallet keypair
- Run ` solana airdrop 2 ` to airdrop sol to your wallet
#### Build
- Clone the repo and change into its root directory
- Run ` anchor build ` to generate a new public key for your program
- Run ` anchor keys list `. Copy the new pubkey into your declare_id!
macro at the top of `lib.rs` and replace the default key in `Anchor.toml`
- Change the `provider.cluster` variable in `Anchor.toml` to `devnet`
#### Deploy and test
- Run ` anchor deploy `
- Run ` anchor run test `








