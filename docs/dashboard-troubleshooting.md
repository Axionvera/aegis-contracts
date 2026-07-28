# Dashboard Local Setup & Troubleshooting Guide

This guide provides practical fixes for common errors encountered when configuring the Aegis RWA dashboard locally against a Soroban network.

## Prerequisites
Ensure your local environment meets the following requirements before starting the Next.js application:
- **Node.js**: `v18.x` or higher
- **NPM / Yarn**: `npm >= 9.x` or `yarn >= 1.22.x`
- **Freighter Wallet**: Installed as a browser extension.

## Environment Variables
The dashboard requires specific environment variables to communicate with the Soroban RPC and the Aegis smart contracts. 

Copy the provided `.env.example` to `.env.local` in the dashboard directory:
```bash
cp .env.example .env.local
```
*Note: Ensure `NEXT_PUBLIC_CONTRACT_ID` exactly matches the address of your deployed Aegis contract.*

---

## Common Errors & Fixes

### 1. Freighter Wallet Errors
**Error:** `Freighter is not installed or not available` or `User declined access`
- **Fix 1:** Ensure the Freighter extension is unlocked and you have selected a network (e.g., Testnet or Futurenet).
- **Fix 2:** If you are testing locally, ensure Freighter is set to allow experimental features or local network connections in its settings (`Settings > Preferences > Allow Experimental Features`).
- **Fix 3:** If the connection modal doesn't appear, check that no other wallet extension (like Albedo or xBull) is intercepting the Stellar wallet API.

### 2. RPC and Contract ID Errors
**Error:** `Transaction failed` or `Error(Contract, 4004)` or `HostError: WasmVm error`
- **Fix 1 (Wrong Network):** Check that `NEXT_PUBLIC_SOROBAN_RPC_URL` matches the network where your `NEXT_PUBLIC_CONTRACT_ID` is deployed. If the contract is on Testnet but the RPC is pointing to Futurenet, calls will silently fail or return cryptic VM errors.
- **Fix 2 (Stale Contract ID):** If you recently re-deployed the contracts, ensure you updated `NEXT_PUBLIC_CONTRACT_ID` in `.env.local` and restarted the Next.js development server.

### 3. Next.js Setup Errors
**Error:** `Module not found: Can't resolve 'soroban-client'` or `BigInt is not defined`
- **Fix 1:** Run `npm install` or `yarn install` to ensure all dependencies are resolved. 
- **Fix 2:** Soroban SDKs rely heavily on `BigInt`. Ensure your `tsconfig.json` specifies `"target": "es2020"` or higher so that BigInt is polyfilled correctly during the Next.js build process.
- **Fix 3:** Clear the Next.js build cache by deleting the `.next` folder and running `npm run dev` again.

### 4. Contract Configuration Errors (4000 / 4001)
**Error:** Transfers or mints fail immediately on submission.
- **Fix:** These are Aegis compliance errors (Sender/Receiver not whitelisted). Ensure your connected Freighter wallet address is explicitly whitelisted on-chain. You can use the Soroban CLI to invoke `whitelist_user` using the Admin account before testing frontend flows.

## Debugging Commands

If you are still experiencing issues, use the following commands to diagnose your environment:

1. **Verify Contract State:**
   ```bash
   soroban contract invoke --id <CONTRACT_ID> --network testnet --source <ADMIN_SECRET> -- get_investor_eligibility --investor <YOUR_FREIGHTER_ADDRESS>
   ```
2. **Clear Next.js Cache:**
   ```bash
   rm -rf .next && npm run dev
   ```
3. **Verify Network Passphrase:**
   Ensure `NEXT_PUBLIC_NETWORK_PASSPHRASE` exactly matches the Stellar network you are targeting (e.g., `Test SDF Network ; September 2015`).
