# Why Brave Android searches our app-launch QR codes

## Finding

The reported failure is explained by **Brave's address-bar QR scanner's URL/search
classification**, not by QR rendering, our APK's native staging receipt, or proof
of a malformed Android intent.

The user reports that both the original `webp2p-controller://…` synthetic QR and
the package-targeted `intent://…#Intent;…;end` synthetic QR open Brave search.
Brave's inspected scanner explicitly sends strings which do not match
`android.util.Patterns.WEB_URL` into its search-engine path. Both of our formats
fail that pattern. Adding an Android package selector cannot change this branch.

**Do not scan a live visitor grant with this scanner.** This path can submit the
credential-bearing string as a search query. The experiments reported here used
the synthetic QR; they do not establish exposure of a real grant.

## Primary sources

Brave source snapshot: `brave/brave-core` commit
`1a3fa018935497d8d6952bacdb28df1751de04a4`. This is an inspected upstream snapshot,
not a claim that the installed phone runs that exact commit. The phone's precise
Brave version was not supplied. Its observed behavior matches this code path.

1. [BraveLocationBarQRDialogFragment.onDetectedQrCode](https://github.com/brave/brave-core/blob/1a3fa018935497d8d6952bacdb28df1751de04a4/android/java/org/chromium/chrome/browser/omnibox/BraveLocationBarQRDialogFragment.java#L150-L184)
   reads `barcode.displayValue` and branches on a **full-string** pattern match:

   ```java
   if (Patterns.WEB_URL.matcher(barcodeValue).matches()) {
       mLocationBarMediator.setSearchQuery(barcodeValue);
       // Then updates/selects the omnibox text.
   } else {
       mLocationBarMediator.performSearchQuery(barcodeValue);
   }
   ```

   There is no `Intent.parseUri` or generic external-app dispatch in this callback.
   Even the successful web-pattern branch is omnibox handling, not a guarantee of
   automatic app launch. Do not confuse this general scanner with Brave Sync or
   Wallet's purpose-specific QR handling.

2. [BraveLocationBarMediator.performSearchQuery](https://github.com/brave/brave-core/blob/1a3fa018935497d8d6952bacdb28df1751de04a4/android/java/org/chromium/chrome/browser/omnibox/BraveLocationBarMediator.java#L222-L241)
   calls `TemplateUrlService.getUrlForSearchQuery(query, null)` and loads the
   returned URL. If no search URL can be generated it falls back to populating the
   omnibox, not launching an Android intent. Thus changing the search provider or
   removing one is not a demonstrated app-launch fix.

3. [Android 16 Patterns.java](https://github.com/aosp-mirror/platform_frameworks_base/blob/android-16.0.0_r1/core/java/android/util/Patterns.java#L356-L386)
   defines `WEB_URL` using a limited web-protocol pattern
   `(?i:http|https|rtsp|ftp)://`, domain/IP rules and path/query rules. It is not an
   arbitrary-URI validator. A custom scheme or Android `intent:` wrapper does not
   become a matching web URL merely because an installed app accepts it.

4. [BraveExternalNavigationHandler.startActivity](https://github.com/brave/brave-core/blob/1a3fa018935497d8d6952bacdb28df1751de04a4/android/java/org/chromium/chrome/browser/externalnav/BraveExternalNavigationHandler.java#L46-L73)
   consults `BravePrivacySettings.PREF_APP_LINKS` before delegating an external
   activity launch. That is a different, later path. Enabling App Links does not
   rewrite the scanner's non-web-string-to-search decision. This does **not** mean
   Brave never supports external apps or intent links in other navigation contexts.

5. [Chrome's Android intent documentation](https://developer.chrome.com/docs/android/intents)
   documents user-activated **web-page anchors**, including fixed package/scheme
   intent syntax and BROWSABLE activities. It does not document Brave QR scanning.
   Our prior inference from that document to Brave's scanner was incorrect.

Brave's official help-center searches for QR codes and App Links did not yield a
promise of arbitrary app-link dispatch from this scanner. Results such as the
[Sync setup guide](https://support.brave.app/hc/en-us/articles/360021218111-How-do-I-set-up-Sync)
concern a different QR workflow. The implementation above supplies the decisive
explanation; this is not based on an unsourced forum workaround.

## Local reproduction

Compiled the actual AOSP `android-16.0.0_r1` `Patterns.java` with a host JDK
(`javac --release 17`). Removed only its build-time
`@android.ravenwood.annotation.RavenwoodKeepWholeClass` annotation so it could
compile outside Android; the regex constants and matching code were unchanged.

A small Java probe evaluated `Patterns.WEB_URL.matcher(value).matches()` for:

- the real helper's `--qr-test-link` output (fixed synthetic grant only);
- the equivalent original custom-scheme data URI;
- an ordinary `https://example.org/viewer` control.

Observed output (labels only; no credential URLs logged):

```text
ordinary HTTPS control: WEB_URL=true; Brave branch=setSearchQuery (omnibox)
original custom-scheme QR: WEB_URL=false; Brave branch=performSearchQuery (search engine)
package-targeted intent QR: WEB_URL=false; Brave branch=performSearchQuery (search engine)
```

This executes the actual AOSP matching implementation, then identifies the branch
from Brave's source. It is **not** execution of Brave on the phone, an Android
intent-resolution test, or proof about all scanners/browser versions. The user's
two phone observations supply the corresponding end-to-end symptom.

## Consequences for this project

- The direct **terminal QR → Brave address-bar scanner → APK** plan is not working
  for either implemented URI format. Stop treating another URI spelling, an App
  Links toggle or an APK rebuild as a fix for this classification.
- The stale-native Gradle receipt is a separate build-integrity issue. Refreshing
  it does not change Brave's source or QR classification.
- Retain synthetic data for any further launch experiments. Do not try to disguise
  the credential as an HTTPS URL, use a shortener, or introduce a redirect service
  without a separately reviewed design; those were not authorized here.
- A separately verified app-link-capable scanner/launcher, or a local scanner
  inside the APK, would avoid this specific Brave scanner path. Neither is
  established by this research. In-app scanning would require explicit approval
  for camera permission/dependencies and must retain validation/prefill-only and
  manual Start. No such implementation or permission change was made here.

This research changed documentation only at its checkpoint. Subsequently, the user
approved an in-app local camera scanner and compact descriptor. The helper now
emits `wpa2:` compact data for that scanner, not an intent envelope. This does not
change Brave's search classification; do not scan live grants there. See
[the current operator guide](../../apps/android-controller/README.md) for
implementation and evidence limits.
