# ODX Protocol - Technical Documentation

## Overview

ODX is a blue-chip crypto wrapper protocol that enables users to gain exposure
to major cryptocurrencies (ETH, SOL, XRP, ADA) through synthetic assets
(xAssets) backed 1:1 by real assets held in secure custody. The protocol
leverages Sonic's high-performance EVM chain for transaction execution and ICP's
chain-key cryptography for secure multi-chain asset custody.

## Core Components

### 1. Sonic Chain Layer (EVM Component)

- **xAsset Smart Contracts**: ERC-20 compatible wrapped assets (xETH, xSOL,
  xXRP, xADA) deployed on Sonic
- **ODX Protocol Contracts**: Implementation of UniswapX Dutch Order v2 for
  gasless, MEV-protected order fulfillment
- **USDC Settlement**: Users pay and receive USDC on Sonic for minting and
  burning operations

### 2. ICP Custody Layer

- **Chain-Key Cryptography**: Enables ICP canisters to directly control native
  addresses on external blockchains (Bitcoin, Ethereum, Solana, Cardano, Ripple)
- **Multi-Signature Canister**: Custodies all native assets with approval
  required from ODX Network Participants
- **Cross-Chain Security**: Assets remain secure on their native chains while
  controlled by the decentralized ICP canister

### 3. Merchant Network

- **Order Fulfillment**: Merchants monitor and fulfill user orders via Dutch
  auction mechanism
- **Asset Procurement**: Merchants source assets from CEXs, DEXs, or other
  liquidity venues
- **Custody Submission**: Merchants deposit procured assets to ICP
  canister-controlled addresses
- **Proof Submission**: Merchants submit publicly verifiable proofs of custody
  to ODX protocol

### 4. ODX Network Participants

- **Multi-Sig Signers**: Decentralized set of signers controlling the ICP
  canister
- **Consensus Requirement**: All participants must approve before funds can be
  withdrawn from custody
- **Security Model**: Prevents single points of failure and unauthorized asset
  movements

## Protocol Flow

```mermaid
sequenceDiagram
    participant User
    participant Sonic as Sonic Chain<br/>(ODX Protocol + xAssets)
    participant Merchant as Merchant Network
    participant CEX as External Liquidity<br/>(CEX/DEX)
    participant ICP as ICP Canister<br/>(Chain-Key Custody)
    participant Chains as Native Blockchains<br/>(ETH/SOL/XRP/ADA)
    participant ODX_Net as ODX Network<br/>(Multi-Sig Signers)
    participant Proofs as Off-Chain<br/>(Custody Proofs)

    rect rgb(220, 240, 255)
        Note over User,Proofs: MINTING FLOW: User wants xETH
        User->>Sonic: 1. Create partially signed order<br/>(Pay USDC for xETH)
        Sonic->>Merchant: 2. Broadcast Dutch auction order
        Merchant->>Merchant: 3. Evaluate profitability
        Merchant->>CEX: 4. Procure native ETH
        CEX-->>Merchant: 5. Deliver ETH
        Merchant->>ICP: 6. Deposit ETH to canister address
        ICP->>Chains: 7. Store ETH via chain-key cryptography
        Note over ICP,Chains: ICP directly controls<br/>native blockchain addresses
        Merchant->>Proofs: 8. Submit custody proof (public)
        Merchant->>Sonic: 9. Complete order signature
        Sonic->>Sonic: 10. Verify & mint xETH
        Sonic->>User: 11. Transfer xETH to user
        Sonic->>Merchant: 12. Transfer USDC payment
    end

    rect rgb(255, 240, 240)
        Note over User,Proofs: BURNING FLOW: User exits xETH position
        User->>Sonic: 1. Initiate burn of xETH
        Sonic->>Merchant: 2. Broadcast burn order
        Merchant->>Sonic: 3. Accept burn order
        Sonic->>Sonic: 4. Burn xETH tokens
        Sonic->>User: 5. Send USDC at market rate
        Merchant->>ODX_Net: 6. Request withdrawal approval
        ODX_Net->>ODX_Net: 7. Multi-sig consensus
        ODX_Net->>ICP: 8. Approve withdrawal
        ICP->>Chains: 9. Release ETH from custody
        Chains-->>Merchant: 10. Withdraw native ETH
        Merchant->>CEX: 11. Sell ETH for USDC
    end

    Note over User,Proofs: Key Features:<br/>✓ Gasless transactions via UniswapX Dutch Order v2<br/>✓ MEV protection through Dutch auction<br/>✓ Native asset custody via ICP chain-key cryptography<br/>✓ Public verification of 1:1 backing
```

### Minting xAssets

1. **Order Placement**: User creates a partially signed order on Sonic
   specifying desired asset (ETH/SOL/XRP/ADA) and USDC payment amount
2. **Order Discovery**: Merchant monitors Dutch auction orders and selects
   profitable fulfillment opportunities
3. **Asset Procurement**: Merchant acquires the underlying asset from external
   liquidity sources (CEX/DEX)
4. **Custody Deposit**: Merchant sends native asset to ICP canister-controlled
   address on the respective blockchain
5. **Proof Submission**: Merchant submits custody proof to ODX protocol (managed
   off-chain, made public)
6. **Order Completion**: Merchant signs the order to complete fulfillment
7. **xAsset Minting**: ODX protocol mints corresponding xAsset to user's address
   on Sonic
8. **USDC Transfer**: User's USDC payment is transferred to merchant as
   compensation

### Burning xAssets

1. **Burn Request**: User initiates burn of xAsset on Sonic to exit position
2. **Order Matching**: Merchant takes the opposite side of the burn order
3. **xAsset Burn**: Protocol burns user's xAsset tokens on Sonic
4. **USDC Settlement**: User receives USDC at current market rate
5. **Asset Withdrawal**: Merchant withdraws underlying asset from ICP canister
   (requires ODX Network Participants approval)
6. **Market Sale**: Merchant sells withdrawn asset on external markets to
   replenish USDC reserves

## Key Features

### Gasless Transactions

UniswapX Dutch Order v2 implementation enables users to submit orders without
paying gas fees upfront, improving UX and accessibility.

### MEV Protection

Dutch auction mechanism and off-chain order matching protect users from
front-running and sandwich attacks common in DeFi protocols.

### Decentralized Custody

ICP's chain-key cryptography enables trustless custody of native assets without
centralized custodians or wrapped token bridges.

### Multi-Chain Native Support

Unlike traditional bridge-based wrappers, ODX custodies actual native assets
(real ETH, SOL, XRP, ADA) rather than bridge-wrapped versions.

### Transparent Verification

All custody proofs are publicly available, enabling users to verify 1:1 backing
of xAssets at any time.

## Future Roadmap

### Cross-Chain Expansion

ODX plans to become chain-agnostic, allowing users to mint and burn xAssets from
any supported blockchain while maintaining unified custody on ICP.

### Additional Asset Support

Protocol will expand beyond blue-chip cryptocurrencies to support additional
high-quality digital assets based on merchant network capacity and user demand.

## Security Model

**Asset Custody**: All underlying assets are held in ICP canister addresses
secured by chain-key cryptography, requiring consensus from ODX Network
Participants for any withdrawals.

**Order Execution**: Dutch auction mechanism ensures competitive pricing while
protecting users from MEV exploitation.

**Merchant Incentives**: Merchants are economically incentivized to fulfill
orders honestly as they profit from the spread between procurement cost and USDC
payment.

**Public Verifiability**: Custody proofs and canister state are publicly
auditable, enabling community oversight of protocol solvency.

## Technical Stack

- **Smart Contract Chain**: Sonic (high-performance EVM L1)
- **Custody Infrastructure**: Internet Computer Protocol (ICP)
- **Order Protocol**: UniswapX Dutch Order v2 (custom implementation)
- **Settlement Currency**: USDC
- **Supported Assets**: ETH, SOL, XRP, ADA (with more planned)

---

_For more information about ODX Protocol, visit [odx.so] or join our community
at [https://discord.gg/odex]_
