# EFB charts: Navigraph, ChartFox, official AIP PDFs, AMDB maps

Date: 2026-10-22. Crate: `crates/charts` (own `[workspace]`, not yet wired into the plugin).

## 1. What exists today (audit)

| Piece | File | What it does |
|---|---|---|
| Local charts | `src/charts.rs` `local()` | Lists PDF/PNG/JPG/WEBP under `Documents/Charts/<ICAO>`; subfolders become sections. |
| FAA d-TPP | `src/charts.rs` `faa()` | Downloads `d-tpp_Metafile.xml` per AIRAC cycle with `curl` into `%LOCALAPPDATA%/XPHFBW/faa/<cycle>`, lists PDFs with categories. |
| OIT route | `src/oit_files.rs` `GET /oit/charts?icao=&source=faa|local` | Wraps the two functions above for the OIT. |
| App route | `app/src/web.rs` `GET /api/charts` | Same for the EFB app (`source=faa|local`). |
| AMDB Bridge charts | `src/panel.rs` `/study/charts` -> `oans::plugin::charts_index` | Proxies an external "AMDB Bridge" (`/efb/flybywire-aircraft-a380-842/v2/charts/<ICAO>`) that returns Navigraph-shaped chart objects (`image_day_url`, `image_night_url`, `type_code`). |
| AMDB layers | `src/panel.rs` `/study/amdb` | GeoJSON layers from the local AMDB folder (`oans::plugin::data_root()`, default `Output/fbw-a380x/amdb/<ICAO>.json`, EPSG:4326) or the Hugging Face fallback (`src/oans/hf.rs`). |
| Navigraph proxy | `src/navigraph.rs` | Forwards `*.navigraph.com` requests from the screens; injects `client_id`/`client_secret` from `navigraph.ini` (plugin folder or `Output/preferences/navigraph.ini`) into `/connect/*` POSTs. No token storage of its own (the JS side holds tokens). |
| EFB UI | `app/ui/index.html` `loadCharts`/`showChart` (~line 4775) | Source picker (amdb, faa, local); groups by `category || type_code`; renders `kind == "pdf"` in an `<iframe>`, else `<img>` of `url || image_day_url/night_url`; night mode = CSS invert unless a night image exists. No own-ship, no georef. |
| Native OIT page | `crates/oit_pages/src/flt_ops.rs` `ChartsModel` | Sources segment NAVIGRAPH / FAA / MY CHARTS, TERMINAL/ENROUTE, grouped tree, Image or Pdf stage, tools - + FIT ROTATE NIGHT. Model only; no data path yet. |
| FBW reference | `fbw-common/.../EFB/Navigation/Navigation.tsx:190-205` | Own-ship on georeferenced Navigraph charts: inside `planview.latlng`, x = dx*(lon-lng1)/dLon, y = dy*(lat2-lat)/dLat, drawn at (x1, y2). |
| FBW reference | `fbw-common/.../EFB/Apis/ChartFox/ChartFox.tsx` | Legacy ChartFox v1: `POST chartfox.org/api/charts/grouped/<ICAO>?token=`, groups "0","1" reference, "2" airport, "6" SID, "7" STAR, "8" approach. |

What the UI needs from any source: a list with `name`, `category`, `kind` (pdf/image/native/link), something to fetch bytes from, optionally day/night variants and a georef box for own-ship.

## 2. External APIs

### Navigraph (client ID applied for; cannot be tested yet)

Source: developers.navigraph.com (authentication/device-authorization, authentication/pkce, charts/airport-charts), read 2026-10-22.

- Device authorization: `POST https://identity.api.navigraph.com/connect/deviceauthorization`, form: `client_id`, `client_secret`, `code_challenge` (S256), `code_challenge_method=S256`. Reply: `device_code`, `user_code`, `verification_uri`, `verification_uri_complete`, `expires_in`, `interval`.
- PKCE: verifier = base64url(32 random bytes) (43 chars), challenge = base64url(SHA-256(verifier)), no padding.
- Poll: `POST /connect/token`, `grant_type=urn:ietf:params:oauth:grant-type:device_code`, `device_code`, `code_verifier`, `client_id`, `client_secret`, `scope=openid charts offline_access`. Errors (400): `authorization_pending` (wait interval), `slow_down` (+5 s from now on), `access_denied`, `expired_token`.
- Tokens: `access_token` (JWT, ~1 h), `refresh_token` (long-lived, single use: always store the new one). Refresh: `grant_type=refresh_token`.
- Charts: `GET https://api.navigraph.com/v2/charts/<ICAO>?version=STD&rules=IFR` (Bearer). Chart fields: `id`, `name`, `index_number`, `category` (APT/DEP/ARR/APP/REF), `image_day_url`, `image_night_url`, `thumb_*`, `is_georeferenced`, `bounding_boxes.planview.{pixels{x1,y1,x2,y2},latlng{lat1,lng1,lat2,lng2}}`, `bounding_boxes.insets[].pixels` (not to scale), `procedures`, `runways`. Images: GET the URL with the same Bearer.
- Subscription: `subscriptions` claim in the access token contains `charts`; without it the API serves demo images (NZWN, YBBN free).
- Terms: charts must not be stored, cached or available offline; fetch on display. Watermark "This chart is linked to Navigraph account <preferred_username>" (`/connect/userinfo`). Online use inside a virtual cockpit only.
- Wiring: the existing `navigraph.ini` client credentials are reused.

### ChartFox

- Legacy v1 (what FBW's code uses): `POST https://chartfox.org/api/charts/grouped/<ICAO>?token=<secret>`; deprecated by the 2024 relaunch.
- v2 (2024 relaunch, "public API, apply from account settings"): UNVERIFIED. No public docs could be reached (developers.chartfox.org does not resolve; chartfox.org has no API links; api.chartfox.org redirects to chartfox.org). Third-party notes mention `GET https://api.chartfox.org/v2/airport(s)/<ICAO>...` with `Authorization: Bearer`. The crate uses `GET {base}/v2/airports/<ICAO>/charts` and `GET {base}/v2/charts/<id>` (base configurable), parses both the legacy grouped shape and a flat `data[]` list (`type_key`/`type`/`category`), and sends the token only to its own host. Fix the paths once access is granted.
- Auth: user-supplied access token (personal token or OAuth token from a later sign-in flow). Caching: off until the terms are read.

### Free official AIP chart PDFs

`verified`: yes = URL pattern checked against the live site 2026-10-22; partial = index structure seen, chart path not; no = from memory, check before shipping. Kinds: `faa_dtpp` (XML index), `html_index` (fetch a page, keep `.pdf` links), `portal` (one "open in browser" entry), `none` (not free).

| Country | Authority | ICAO prefixes | Kind | URL pattern | Access | Verified |
|---|---|---|---|---|---|---|
| United States | FAA d-TPP | K, PA, PF, PO, PP, PH, PG, TJ, TI | faa_dtpp | `https://aeronav.faa.gov/d-tpp/{cycle}/xml_data/d-tpp_Metafile.xml`, charts `.../d-tpp/{cycle}/{pdf}` | free | yes (already in use) |
| Spain | ENAIRE | LE, GC | html_index | index `https://aip.enaire.es/AIP/AIP-en.html`, charts `contenido_AIP/AD/AD2/{ICAO}/LE_AD_2_{ICAO}_{TYPE}_{n}_en.pdf` (TYPE = ADC, PDC, GMC, SID, STAR, IAC, ...) | free | yes |
| United Kingdom | NATS AIS | EG | html_index | `https://www.aurora.nats.co.uk/htmlAIP/Publications/{airac_iso}-AIRAC/html/eAIP/EG-AD-2.{ICAO}-en-GB.html` | free | no |
| Netherlands | LVNL | EH | html_index | `https://eaip.lvnl.nl/{airac_iso}-AIRAC/html/eAIP/EH-AD-2.{ICAO}-en-GB.html` | free | no (site returned 403 to the fetcher) |
| Belgium, Luxembourg | skeyes | EB, EL | html_index | `https://ops.skeyes.be/html/belgocontrol_static/eaip/eAIP_Main/html/eAIP/{EB|EL}-AD-2.{ICAO}-en-GB.html` | free | no |
| France | SIA | LF | html_index | `https://www.sia.aviation-civile.gouv.fr/media/dvd/eAIP_{DD MON YYYY}/FRANCE/AIRAC-{airac_iso}/html/eAIP/FR-AD-2.{ICAO}-fr-FR.html` | free | no |
| Finland | Fintraffic ANS | EF | html_index | `https://www.ais.fi/eaip/{DD MON YYYY}_{YYYY_MM_DD}/...` issue folder seen; chart page path guessed | free | partial |
| Estonia | EANS | EE | html_index | `https://eaip.eans.ee/{airac_iso}/html/eAIP/EE-AD-2.{ICAO}-en-GB.html` | free | no |
| Poland | PANSA | EP | html_index | `https://www.ais.pansa.pl/eaip/{airac_iso}/html/eAIP/EP-AD-2.{ICAO}-en-GB.html` | free | no |
| Australia | Airservices DAP | Y | html_index (section by `({ICAO})`) | `https://www.airservicesaustralia.com/aip/current/dap/AeroProcChartsTOC.htm` | free | no |
| Norway | Avinor | EN | portal | `https://ais.avinor.no/` | free | no |
| Sweden | LFV | ES | portal | `https://aro.lfv.se/content/eaip/default_offline.html` | free | no |
| Denmark, Greenland, Faroe | Naviair | EK, BG | portal | `https://aim.naviair.dk/` | free | no |
| Ireland | AirNav Ireland | EI | portal | `https://www.airnav.ie/` | free | no |
| Iceland | Isavia ANS | BI | portal | `https://eaip.isavia.is/` | free | no |
| Czechia | ANS CR | LK | portal | `https://aim.rlp.cz/` | free | no |
| Portugal | NAV Portugal | LP | portal | `https://ais.nav.pt/` | free | no |
| Greece | HCAA | LG | portal | `https://aisgr.hasp.gov.gr/` | free | no |
| New Zealand | AIP NZ | NZ | portal | `https://www.aip.net.nz/` (disclaimer redirect first) | free | partial (redirect seen) |
| Canada | NAV CANADA (CAP) | CY, CZ | portal | `https://www.navcanada.ca/en/aeronautical-information/` | free | no |
| Hong Kong | CAD | VH | portal | `https://www.ais.gov.hk/` | free | no |
| Singapore | CAAS | WS | portal | `https://aim-sg.caas.gov.sg/` | free | no |
| India | AAI | VA, VE, VI, VO | portal | `https://aim-india.aai.aero/` | free | no |
| UAE | GCAA | OM | portal | `https://www.gcaa.gov.ae/` | free | no |
| South Africa | SACAA | FA | portal | `https://www.caa.co.za/` | free | no |
| Argentina | ANAC | SA | portal | `https://ais.anac.gob.ar/` | free | no |
| Chile | DGAC | SC | portal | `https://aipchile.dgac.gob.cl/` | free | no |
| Brazil | DECEA AISWEB | SB, SD, SI, SJ, SN, SS, SW | portal | `https://aisweb.decea.mil.br/` (API needs a free key) | free key | no |
| Germany | DFS | ED, ET | none | IFR charts not free | - | no |
| Italy | ENAV | LI | none | registration | - | no |
| Japan | AIS Japan | RJ, RO | none | registration | - | no |
| Switzerland | skyguide | LS | none | registration | - | no |

The table lives in `crates/charts/data/aip_sources.json`; promoting a `portal` row to `html_index` is a data change. Category tokens (SID, STAR, ADC, IAC, ...) are in the same file. Placeholders: `{icao}`, `{prefix2}`, `{cycle}`, `{airac_iso}`, `{airac_compact}`, `{airac_dmy}`, `{airac_us}`.

### AMDB

The OANS's local folder (`oans::plugin::data_root()`), one EPSG:4326 GeoJSON `AmdbResponse` per `<ICAO>.json`. `AmdbSource` returns one "Airport map" entry (`kind: native`) and an `AmdbDescriptor` (path, projection, layer list = the EFB's pushback layers). The native renderer draws it; own-ship needs no georef box since the data is lat/lon.

## 3. Crate design (`crates/charts`, done)

- `ChartSource` trait: `search(icao) -> Vec<Chart>`, `fetch(chart, night) -> ChartData` (`Bytes`, `Native(AmdbDescriptor)`, `Link(url)`), `cacheable()`.
- `Chart`: source, id, icao, name, `Category` (APT/DEP/ARR/APP/REF/OTHER), `ChartKind` (pdf/png/jpeg/webp/native/link), day/night URL (kept inside the plugin; `to_json()` omits them), `Georef` with `project(lat, lon)` (FBW formula, insets excluded), procedures, runways.
- `HttpClient` trait (`send(HttpRequest) -> HttpResponse`); `ScriptedHttp` for tests. The plugin adapter will wrap `crate::net::request_bytes`.
- `NavigraphSource`: device flow with PKCE (own SHA-256/base64url, `getrandom` for entropy), polling state machine honouring `interval`/`slow_down`, refresh 60 s before expiry and once on 401, rejected refresh signs out, single-use refresh tokens replaced, subscription check from the JWT, `username()` for the watermark, refuses non-navigraph image hosts, `cacheable() = false`.
- Token storage: `TokenStore` trait; `FileTokenStore` writes `masked1:` + base64url(XOR with SHA-256 of a machine salt). Obfuscation only; `Debug` masks tokens.
- `ChartfoxSource`: bearer token, configurable base, both list shapes, resolves chart URL via `/v2/charts/<id>` when the list has none, `cacheable() = false`.
- `AipSource`: table-driven; FAA metafile parser (moved from `src/charts.rs`), HTML link scan with optional section, URL joining, dedup, token categoriser, index pages memoised in memory, `cacheable() = true`.
- `AmdbSource`: descriptor only.
- `DiskCache`: SHA-256-named files + content type, LRU by mtime, size cap (default 512 MB, GENERIC), refuses items larger than the cap. `Charts` registry applies it only to cacheable sources.
- `airac`: cycle number and effective-date placeholders.

Tests: 29 (8 unit, 21 integration with scripted replies and anonymised fixtures under `tests/fixtures`).

## 4. Remaining work (not in this crate's brief)

1. Plugin wiring (shared files, separate brief): add `charts` as a path dependency; an `HttpClient` over `crate::net::request_bytes` (blocking, run off the main thread like `net::poll_with`); routes `GET /charts/sources`, `GET /charts/list?source=&icao=`, `GET /charts/file?source=&icao=&id=&night=` (bytes, or JSON for native/link), `POST /charts/navigraph/signin` (returns user code + QR URL), `GET /charts/navigraph/poll`, `POST /charts/navigraph/signout`. Keep `/oit/charts` and `/api/charts` answering through the same registry so FAA/local keep working.
2. Navigraph token file: `Output/preferences/a380_navigraph.tok` via `FileTokenStore::for_this_machine`. Decide whether the JS screens' own Navigraph session (via `src/navigraph.rs`) and this one should share tokens.
3. EFB/OIT UI: add NAVIGRAPH, CHARTFOX, AIP, AMDB to the source picker; show device code + `verification_uri_complete` (QR) for sign-in; Navigraph watermark in the margin; own-ship marker from `Chart.georef.project()` with live lat/lon; AMDB entry opens the native airport map renderer.
4. ChartFox: confirm v2 endpoints and terms once API access is granted; add an OAuth sign-in if personal tokens are not offered.
5. AIP table: verify each `no` row on the live site, promote portals to `html_index` where an eAIP page lists PDFs, add more countries; FAA metafile (~10 MB) should go to the disk cache rather than memory.
6. Local charts (`Documents/Charts`) can become a fifth `ChartSource` later; it already works through `src/charts.rs`.

Numbers marked GENERIC: cache cap 512 MB, refresh margin 60 s, in-memory index page limit 16.
