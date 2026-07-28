# Emergency Pause Policy

This document defines the emergency pause mechanism for the Aegis RWA Protocol, including what it affects, who can invoke it, and the trust model.

## Overview

The Aegis contracts implement a global emergency pause that blocks all state-changing operations when activated. This is designed as a **kill switch** for regulatory incidents, key compromise, or protocol vulnerabilities.

## Pause Scope

When the contract is paused, the following operations are **blocked**:

| Operation | Module | Blocked |
|---|---|---|
| `mint_asset` | asset | Yes |
| `transfer` | asset | Yes |
| `distribute_yield` | asset | Yes |
| `whitelist_user` | compliance | Yes |
| `revoke_whitelist` | compliance | Yes |
| `set_role` | admin | Yes |
| `remove_role` | admin | Yes |
| `transfer_admin` | admin | Yes |
| `accept_admin` | admin | Yes |
| `renounce_admin` | admin | Yes |

The following operations **remain available** during a pause:

| Operation | Module | Available |
|---|---|---|
| `is_paused` | admin (read) | Yes |
| `get_role_of` | admin (read) | Yes |
| `get_balance_of` | lib (read) | Yes |
| `get_total_supply` | lib (read) | Yes |
| `is_whitelisted` | lib (read) | Yes |
| `pause` | admin | Yes |
| `unpause` | admin | Yes |

Read functions are never blocked by a pause. This ensures that dashboards, frontends, and off-chain monitoring systems can continue to query contract state even during an emergency.

## Authorization

### Pause

Anyone with the **Admin** role or the **EmergencyOfficer** role can pause the contract. This dual-authorisation ensures that either the supreme admin or a designated emergency responder can halt operations.

- The caller must have `Admin` or `EmergencyOfficer` role
- The contract must not already be paused
- Emits a `contract_paused` event

### Unpause

Only the **Admin** can unpause the contract. This is a deliberate design decision: an EmergencyOfficer can halt the protocol but cannot resume it. Only the supreme admin (or a governance multisig) can restore normal operations.

- The caller must be the Admin
- The contract must be paused
- Emits a `contract_unpaused` event

## Events

| Event | Payload | When |
|---|---|---|
| `contract_paused` | `{ admin: Address }` | Contract is paused |
| `contract_unpaused` | `{ admin: Address }` | Contract is unpaused |

## Trust Model

The pause system concentrates significant power in two roles:

1. **Admin**: Can pause, unpause, and control all roles. This is the highest-privilege key.
2. **EmergencyOfficer**: Can pause but not unpause. Designed for a separate, operational key that can react to incidents without needing full admin access.

### Key risks

- **Admin key compromise**: The attacker can pause and unpause at will, or simply ignore the pause and continue operating with full admin privileges.
- **EmergencyOfficer key compromise**: The attacker can pause the contract but cannot unpause it. This is a denial-of-service risk but not a fund-theft risk.
- **Admin unavailable**: If the admin key is lost and the contract is paused, the protocol is permanently halted. Admins should use hardware wallets or multi-sig solutions.

### Recommendations

1. Use a **hardware wallet or multi-sig** for the admin key.
2. Assign the EmergencyOfficer role to a **separate, secured key** (e.g., an incident response team key).
3. Test pause/unpause flows on testnet before mainnet deployment.
4. Document the admin key recovery procedure off-chain.
5. Set up off-chain monitoring for `contract_paused` and `contract_unpaused` events.

## Comparison with other pause designs

Some protocols implement per-operation pauses (e.g., pausing minting but not transfers). The Aegis contract uses a **single global pause** for simplicity and maximum safety. In an emergency, partial pauses may not cover all attack vectors, and a global pause ensures no state-changing operations can proceed.

If finer-grained pause control is needed in the future, it can be added as a separate module without breaking the existing pause interface.
