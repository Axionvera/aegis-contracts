# Legal Boundary Disclaimer

> [!WARNING]
> **This protocol enforces technical state, not legal reality.** 

The Aegis RWA Contracts provide the on-chain infrastructure for tokenizing Real-World Assets (RWAs). However, these smart contracts **do not replace, bypass, or guarantee** real-world legal and regulatory processes. 

By utilizing or interacting with this codebase, you acknowledge the following critical boundaries:

## Protocol-Level Compliance Limits
The smart contracts strictly enforce on-chain ledger state according to the parameters set by the protocol administrators:
- **Whitelisting**: The contracts enforce that tokens can only be minted to or transferred between addresses registered on the on-chain whitelist.
- **Caps**: The contracts enforce supply and per-investor holding caps as configured mathematically on the ledger.
- **State Blocking**: The emergency pause mechanism halts on-chain state changes.

**The protocol does not natively know who owns an address, what jurisdiction they are in, or whether a transaction is legally permissible.** It only knows if the address boolean flag `whitelist` is true or false.

## KYC, AML, and Legal Process Assumptions
The protocol architecture operates under the assumption that a fully independent, off-chain entity (such as a compliance provider, broker-dealer, or qualified custodian) is responsible for:
- Performing all KYC (Know Your Customer) and AML (Anti-Money Laundering) checks.
- Mapping real-world legal identities to blockchain addresses.
- Determining eligibility before invoking the `whitelist_user` or `revoke_whitelist` contract endpoints via the `ComplianceOfficer` role.

The smart contract itself performs **zero** identity verification.

## RWA Off-Chain Limitations
The tokens minted via these contracts are cryptographic representations of Real-World Assets. 
- **Asset Backing**: The contract does not hold, secure, or verify the physical or financial assets backing the token.
- **Legal Rights**: Ownership of the token on-chain does not automatically confer real-world legal ownership, dividend rights, or voting rights without a corresponding off-chain legal framework and offering memorandum.
- **Redemptions**: Token redemption for underlying fiat or assets is executed entirely off-chain by the asset issuer or custodian.

**Documentation within this repository regarding "compliance", "restrictions", or "investor eligibility" refers strictly to the algorithmic constraints programmed into the Soroban environment, and should not be interpreted as legal advice or comprehensive regulatory compliance.**
