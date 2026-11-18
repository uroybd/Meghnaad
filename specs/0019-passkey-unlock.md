# 0019: Unlock the static web app with a passkey

- **Status:** in progress <!-- draft | ready | in progress | done | dropped -->
- **Created:** 2026-11-19
- **Layer:** 2 (UI)
- **Front ends:** web, the static flavour only (the Worker flavour signs in with Cloudflare Access; the phone keeps its
  key in the Keystore)
- **Supersedes / related:** 0013 (the sealed access and the unlock form), 0018 (trust this device: a passkey is how a
  trusted device unlocks after *Lock now*), 0017 (reminders while locked)

## Why

Unlocking the static web app means typing the encryption secret: long, the same one `task` uses, and asked for on every
visit and after every idle lock. A passkey (Face ID, a fingerprint, a security key, or a password manager such as
Bitwarden) is quicker and never typed where it can be watched or logged. The app has no server to sign in to, so the
passkey is not a login: it unlocks what this device keeps, using the WebAuthn PRF extension, which gives the page a
secret only that passkey can produce.

The encryption secret stays. It connects a new device, and it unlocks wherever the passkey does not work (a browser or
password manager without PRF, a passkey that is gone), so a passkey that sometimes fails costs nothing but a typed
password.

## What

1. **Setting it up.** Once unlocked, *Settings → Device* offers *Unlock with a passkey on this device*. On a trusted
   device (0018) any passkey will do: it is checked by its signature, and stands in for the password after *Lock now*.
   On a device that is not trusted, the passkey must give the PRF secret, which seals this device's access; if it
   cannot, the app says so (*This passkey or browser can't unlock the app here: trust this device to use it, or keep
   using the password*) and keeps nothing.
2. **Unlocking.** The unlock form puts *Unlock with passkey* first on a device that has one, with the password field
   below. A passkey that fails, is cancelled or is not there leaves the form as it is, password ready.
3. **Each device on its own.** What a passkey unlocks is kept on the device that set it up. A passkey synced to another
   device (by Bitwarden, iCloud, Google) unlocks nothing there until that device sets it up too.
4. **Removing it.** *Settings → Device* has *Stop unlocking with a passkey*, which removes what was kept for it (the
   passkey itself stays in the password manager; the app says where to delete it). *Forget this device* removes it with
   everything else.
5. **With 0018.** A trusted device opens unlocked and needs nothing; after *Lock now* it asks for the passkey when there
   is one, else the password.
6. **Where it is not offered.** A browser without WebAuthn never shows the option; on a device that is not trusted,
   neither does one that reports no PRF.

## Not doing

- Passkeys instead of the encryption secret: the secret is still needed to connect, and is the fallback.
- A passkey that connects a new device, or one shared between devices through the branch.
- Signing in to anything: there is no server, and nothing about the passkey goes to the forge.
- More than one passkey per device (set up again to replace it).
- The Worker flavour, the phone and the terminal app.

## Design

Agreed 2026-11-19 (the two kinds of passkey with the person's yes the same day).

- **Two kinds**, kept in the `meghnaad-engine` IndexedDB database under `passkey` (`web/src/engine/vault.ts`), with the
  WebAuthn calls in `web/src/engine/passkey.ts`:
  - **PRF** (a device that is not trusted): a random 32-byte salt for this device; `navigator.credentials.create` with
    `userVerification: 'required'` and `extensions: { prf: { eval: { first: salt } } }` (where `create` gives no
    result, one `get` right after gets it). The PRF output goes through HKDF (SHA-256) to an AES-GCM key that seals the
    `Unlocked` (access and the key of the tasks) with `persist.ts`'s `sealBytes`. Kept: the credential id, the salt and
    the sealed bytes; never the PRF output or the AES key.
  - **Signature** (a trusted device): `create` as above without PRF; kept: the credential id, its public key
    (`getPublicKey()`, SPKI) and algorithm (ES256 or RS256). Unlocking checks the signature over a fresh challenge
    (the authenticator data and the hash of the client data, the challenge, the origin, user present and verified),
    then opens the trusted copy (0018). It protects the screen, not the data: turning trust off removes it.
- **Unlocking** (`session.ts`): `get` with `allowCredentials: [that id]`, then open and `engine.unlockWith`, as the
  password does. A cancelled passkey leaves the form as it is; another failure says why; a seal that will not open is
  removed.
- **Offered** where `PublicKeyCredential` exists; on a device that is not trusted, not where
  `PublicKeyCredential.getClientCapabilities()` reports no PRF (where the browser cannot say, setting up finds out).
- **Decided:** the unlock form waits for the button (no prompt on opening); after *Lock now* a trusted device asks for
  the passkey when there is one, else the password.
- **No change to `tc-core`**, so neither WebAssembly grows.

## Acceptance criteria

- [x] *Unlock with a passkey on this device* in *Settings → Device*, shown only where the browser has WebAuthn and,
      on a device that is not trusted, does not report missing PRF.
- [x] Setting up with a passkey that gives no PRF secret says so and keeps nothing.
- [x] With one set up, the unlock form offers the passkey first and unlocks with it; cancelling or failing leaves the
      password ready.
- [x] A PRF passkey's seal opens only with its secret (the secret and the AES key are never stored); a signature
      passkey opens only the trusted copy, and only after its signature checks out.
- [x] *Stop unlocking with a passkey* and *Forget this device* remove what was kept.
- [ ] Checked by a person in Firefox with Bitwarden, and in one Chromium browser with its own password manager.
- [x] Tests: setting up (supported, not supported), unlocking, a failed or cancelled passkey falling back, removing
      (with WebAuthn stubbed in the tests).
- [x] Docs: `docs/using/` (the static app: unlocking with a passkey), `docs/architecture.md` (what the static flavour
      keeps on a device).

## Open questions

- **Bitwarden in Firefox gives the PRF secret?** Unknown; it decides only whether a device that is not trusted can use
  it there (a trusted one can, by signature). To be seen when a person checks it.

## Implementation notes

Filled in while building: what was done, where, anything that differed from the design and why, sizes measured, and
what is left. Dated entries, newest last.

### 2026-11-19

Built, as the agreed design (both kinds).

- `web/src/engine/passkey.ts` (new): `createPasskey('prf' | 'signature')` (ES256 or RS256, user verification
  required; PRF from `create`, else one `get`), `usePasskey`, `signedHere` (client data type, challenge and origin; the
  site's hash, user present and verified; the signature, ECDSA's DER read as r and s), `prfReported`
  (`getClientCapabilities`), `PasskeyCancelled` (`NotAllowedError`, `AbortError`) and `NoPrf`.
- `vault.ts`: `keepPrfPasskey` (HKDF of the PRF secret to an AES-GCM key; kept: id, salt, sealed), `keepSignaturePasskey`
  (id, public key, algorithm), `keptPasskey`, `openWithPrf`, `forgetPasskey`.
- `session.ts`: `passkeyPossible`, `setUpPasskey` (signature on a trusted device, else PRF), `unlockWithPasskey` (a PRF
  passkey whose seal will not open, or a signature passkey on a device no longer trusted, is removed), `passkeyKind`,
  `removePasskey`.
- `StaticGate.svelte` (*Unlock with passkey* first; cancelled: no message; failed: why; either way the secret's field
  focused) and `DevicePanel.svelte` (set up, stop, where the passkey itself stays).
- Tests: `passkey.test.ts` (10: real P-256 and RSA signatures, each check failing on its own, the browser stubbed for
  making and using), `vault.test.ts`, `session.test.ts`, the gate's and the panel's.
- **Run in a real browser**: headless Chrome with its virtual authenticator (CDP `WebAuthn.addVirtualAuthenticator`), on
  a scratch page with this module: with PRF, the same secret on each use and another for another salt, the signature
  checked and refused for another origin; without PRF, setting up says so and the signature kind still works. Chrome
  reports PRF in `getClientCapabilities` even when the authenticator lacks it, so on a device that is not trusted
  setting up is what finds out.
- **Built together with 0017, 0018 and 0019** (they share `session.ts`, `vault.ts`, the gate and the Device panel).
- **Sizes**: no change to `tc-core`, the engine or the Worker. The static site's JavaScript (Vite build with
  `VITE_FLAVOUR=static`, gzip -9, all chunks) 354,033 → 359,138 bytes (+5.1 KB): the session, vault and passkey code
  now in their own chunk loaded with the gate, and +55 bytes in the app chunk.
- **Checks**: `npm run lint`, `npm run check`, all web tests (923), `mdbook build docs`.
- **Left**: checked by a person (Firefox with Bitwarden, a Chromium browser). Two criteria were reworded to the built design, with the person's yes.
