# Linux Google sign-in and sessions

On Linux, **Account → Continue with Google** opens the system browser and returns through a one-time local OAuth callback protected by PKCE and a single-use server challenge (`nonce`). Sthira's session service (`crates/sthira-session-service`) verifies the Google RS256 ID token (audience, signature, issuer, expiry, verified email, and challenge `nonce`) and confirms the active Firebase Authentication account before atomically consuming the challenge and issuing an opaque device session credential.

- Each Linux installation generates a persistent random device ID and a recognizable default label (`<host> (Linux · <id-prefix>)`) in its app-local data directory.
- Each sign-in issues a separate session ID and high-entropy credential stored in the Linux Secret Service keyring (`app.sthira.desktop.session`); the session service stores only its SHA-256 hash in an auth-only SQLite WAL database independent of personal vault data.
- Legacy pilot refresh-token entries (`app.sthira.desktop.firebase`) never auto-restore or enroll replacement sessions when the session service is active; one explicit Google sign-in saves the new device credential first and removes the legacy entry afterward.
- **Account → Devices** lists active sessions for the verified Firebase UID, marks the current installation (`Current device`), and provides a **Sign out device** action with named confirmation before revoking another device.
- On startup and Account checks, confirmed revocation clears the device's keyring entry, drops in-memory session state, and displays **This device was signed out.** without relaunching Google automatically; a subsequent restart opens cleanly to **Continue with Google**.
- When the session service is unreachable, Sthira preserves the saved keyring credential, keeps the personal vault usable offline, reports that the saved session cannot be verified, and retries automatically on reconnect or via **Retry verification**. Local **Sign out** revokes the active server session best-effort and removes saved keyring credentials.

## Set up the test project

1. Create a Firebase test project and enable Google under Authentication → Sign-in method. Choose a project support email. Register a Firebase web app to obtain the Firebase web API key. Hosting and a shared-workspace database can remain unconfigured and open.
2. In the same Google Cloud project, create a Desktop app OAuth client. If the consent screen is in Testing, add the test Google account to its audience.
3. Add that Desktop client ID to the Google provider's external client IDs. Keep the automatically managed web OAuth client assigned to the Firebase Google provider.
4. Start the disposable session service and launch Sthira against a disposable QA vault:

```sh
export STHIRA_FIREBASE_WEB_API_KEY='firebase-web-api-key'
export STHIRA_GOOGLE_DESKTOP_CLIENT_ID='desktop-client-id.apps.googleusercontent.com'
cargo run -p sthira-session-service -- --listen 127.0.0.1:8787 --db /tmp/sthira-qa-sessions.db
```

Then in another shell:

```sh
export STHIRA_FIREBASE_WEB_API_KEY='firebase-web-api-key'
export STHIRA_GOOGLE_DESKTOP_CLIENT_ID='desktop-client-id.apps.googleusercontent.com'
export STHIRA_SESSION_SERVICE_URL='http://127.0.0.1:8787'
qa_vault="$(mktemp -d)"
cd crates/sthira-desktop
env -u STHIRA_SEED_DEMO STHIRA_VAULT_DIR="$qa_vault" cargo tauri dev
```

If the Desktop client exchange requires its downloaded client secret, supply it locally as `STHIRA_GOOGLE_DESKTOP_CLIENT_SECRET`. Keep it out of commits, logs, screenshots, and reports. Linux sign-in requires a Secret Service-compatible desktop keyring, such as GNOME Keyring.

For a CW1 membership test, start the same service with `--membership-db` pointing at a separate SQLite file in the disposable QA directory. The default service supports sign-in and device management. Team operations require an explicitly configured membership store. This SQLite adapter is for QA while the shared-workspace database and hosting decisions remain open.

## Verification status

Per-device revocation is signed off for the Linux pilot. The latest custom-protocol build passed eight native loopback checks with isolated keyrings, including an ambiguous 401 during remote sign-out. A retained its credential, showed **Retry verification**, and restored the Devices list without reopening Google sign-in. The same run rechecked B's two signed-out restarts and A's continued sign-in. The live Google and Firebase acceptance below used the preceding build. The latest loopback results are in `tmp/verification/cw1-401-review-c50sje/loopback-results.json`.

- **Unit, service, and UI checks:** `sthira-session-service` tests cover independent same-account sessions, cross-UID revocation rejection, stale replacement rejection (`409 stale_device_session`), idempotent revocation, persistence across service restart, challenge expiry/replay, pre-Firebase RS256 OIDC + `nonce` verification (zero Firebase requests on mismatched/missing nonce and fresh post-provider clock rechecks), and concurrent revocation winning over in-flight enrollment. Desktop and UI tests cover PKCE + challenge binding, legacy pilot keyring migration order, terminal-vs-ambiguous `401` handling (preserving saved credentials on ambiguous/HTML/malformed `401`s), device list labeling, named target confirmation, stale replacement handling, revoked/invalid/expired states, and reconnect retry.
- **Native two-profile Linux GUI (loopback OIDC + real session service + dual Secret Service keyrings):** Passed all seven steps (`tmp/verification/cw1-device-revocation/loopback-results.json`, `tmp/verification/cw1-device-revocation/screenshots-loopback/`) against the rebuilt custom-protocol `sthira-desktop` binary, the rebuilt `sthira-session-service` binary, and two isolated persistent GNOME Keyring D-Bus sessions—covering two-device enrollment/restart, stale replacement conflict rejection before confirmation, revoking B from A, B online revocation detection/keyring cleanup + second-restart clean state, service restart credential rejection (`401 session_revoked`) while keeping A active (`200 OK`), offline outage preservation/reconnect cleanup, and explicit re-sign-in on B.
- **Live two-profile Linux GUI against Sthira Auth QA:** Passed end-to-end against **Sthira Auth QA** (`tmp/verification/cw1-device-revocation/live-results.json`, `tmp/verification/cw1-device-revocation/screenshots-live/`) using the rebuilt desktop and session-service binaries, real Google OAuth browser consent, real Firebase Authentication, and two isolated vaults, installations, and persistent GNOME Keyring D-Bus sessions. Profile A revoked Profile B cleanly while staying signed in (before and after A's own restart); Profile B detected revocation, cleared its keyring, showed **This device was signed out.**, stayed signed out across both its first and second restarts without relaunching Google automatically, and verified `0` saved keyring entries on B while A retained `1`. Single-device live Firebase sign-in/restore/sign-out remains recorded separately in `tmp/verification/cw1-firebase-sessions/results.json`.

[Google desktop OAuth](https://developers.google.com/identity/protocols/oauth2/native-app) · [Google provider setup and external client IDs](https://docs.cloud.google.com/identity-platform/docs/web/google) · [Firebase session API](https://firebase.google.com/docs/reference/rest/auth/)

## Browser build

The React/Vite interface can also be deployed as a static site. The current web build is a preview with synthetic fallback data. A hosted team client would use Firebase's browser sign-in flow and a shared-workspace API. The native desktop continues handling personal SQLite vaults, local files, and local integrations. The shared-workspace database and hosting decisions remain open.

[Firebase Hosting for single-page apps](https://firebase.google.com/docs/hosting/)
