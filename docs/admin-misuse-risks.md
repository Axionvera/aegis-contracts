# Admin Misuse Risk Assessment

This document catalogues the risks associated with admin privileges in the Aegis RWA Protocol and the mitigations implemented to address them.

## Risk Categories

### 1. Single Point of Failure (Admin Key Compromise)

**Risk**: If the admin's secret key is compromised, an attacker gains full control over the contract — including minting tokens, managing the whitelist, and transferring admin rights.

**Mitigations**:
- **2-step admin transfer** (`transfer_admin` / `accept_admin`): Prevents accidental or malicious admin loss. The candidate must explicitly accept the role.
- **Role delegation**: The admin can assign scoped roles (ComplianceOfficer, AssetManager) to separate keys, limiting the blast radius of any single key compromise.
- **Emergency role revocation**: The admin can immediately revoke any role using `remove_role`.
- **Off-chain monitoring**: All role changes emit Soroban events, enabling real-time alerting on suspicious admin activity.

**Residual Risk**: If the admin key is compromised before a transfer is initiated, the attacker can mint tokens or modify the whitelist. Contract deployers should use hardware wallets or multi-sig solutions for the admin key.

### 2. Unauthorized Minting

**Risk**: An attacker with mint privileges can inflate the token supply, devaluing existing holders.

**Mitigations**:
- **Role-based access**: Only addresses with the `AssetManager` role (or the admin) can mint.
- **Whitelist enforcement**: Tokens can only be minted to whitelisted addresses.
- **Positive amount check**: Minting requires `amount > 0`.
- **Off-chain auditing**: The `TotalSupply` counter and mint events enable independent verification of supply changes.

**Residual Risk**: An AssetManager with malicious intent could mint to a whitelisted accomplice. The admin should only assign this role to trusted entities.

### 3. Whitelist Abuse

**Risk**: A ComplianceOfficer could whitelist unauthorized addresses, enabling them to receive or transfer tokens.

**Mitigations**:
- **Scoped role**: Only ComplianceOfficer, EmergencyOfficer, or Admin can whitelist users.
- **Revocation capability**: The admin can revoke any ComplianceOfficer role at any time.
- **Revoke whitelist**: ComplianceOfficers can also remove users from the whitelist, enabling rapid response to compliance violations.

**Residual Risk**: A ComplianceOfficer could whitelist a address and immediately transfer tokens before revocation. Off-chain compliance monitoring should detect and alert on suspicious whitelist additions.

### 4. Admin Renunciation (Irreversible)

**Risk**: An admin can call `renounce_admin`, permanently removing admin access. This could be used maliciously to prevent future upgrades or emergency actions.

**Mitigations**:
- **Clear documentation**: The `renounce_admin` function is documented as irreversible.
- **Event emission**: Renunciation emits an event for off-chain alerting.
- **No automatic role cleanup**: Only the Admin role is removed; other roles assigned by the admin remain active.

**Residual Risk**: If the admin renounces without transferring to a new admin first, the contract becomes permanently ungovernable. Admins should always transfer before renouncing.

### 5. Role Stacking

**Risk**: The current design assigns exactly one role per address. An address cannot hold multiple roles simultaneously.

**Mitigation**: This is intentional — it simplifies auditing and reduces the attack surface. If an address needs both compliance and asset privileges, assign the `EmergencyOfficer` role.

### 6. Stale Role Assignments

**Risk**: Roles assigned to addresses that are no longer active or trusted remain effective until explicitly revoked.

**Mitigations**:
- **Proactive revocation**: The admin should regularly audit and revoke roles for inactive addresses.
- **Event-based monitoring**: Role assignment and revocation events enable off-chain tracking of active roles.
- **No automatic expiry**: Soroban does not support time-based storage expiry for persistent entries, so roles persist until manually revoked.

## Recommendations for Contract Deployers

1. **Use a hardware wallet or multi-sig** for the admin key. Never store the admin secret key in software wallets or hot storage.
2. **Assign minimal roles**: Only grant the specific role needed for each operational address. Do not use `EmergencyOfficer` unless both compliance and asset management are required from the same key.
3. **Monitor events**: Set up off-chain listeners for `role_assigned`, `role_revoked`, `admin_transfer_initiated`, and `admin_transferred` events.
4. **Test role changes on testnet** before deploying to mainnet.
5. **Document the admin key recovery procedure** off-chain in a secure, access-controlled location.
6. **Plan for admin transfer early**: Initiate the 2-step admin transfer as part of the deployment ceremony, transferring to a dedicated governance multisig.

## Threat Model Summary

| Threat | Severity | Likelihood | Mitigation |
|---|---|---|---|
| Admin key compromise | Critical | Low | Hardware wallet, 2-step transfer, role delegation |
| Unauthorized minting | Critical | Low | Role-based access, whitelist enforcement |
| Whitelist abuse | High | Medium | Scoped role, revocation capability, off-chain monitoring |
| Admin renunciation | Medium | Low | Documentation, event alerting |
| Stale role assignments | Medium | Medium | Proactive revocation, event monitoring |
| Role stacking confusion | Low | Low | Single-role-per-address design |
