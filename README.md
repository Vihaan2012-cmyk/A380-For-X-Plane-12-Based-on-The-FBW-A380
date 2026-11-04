# A380 for X-Plane 12

A free, unofficial port of the [FlyByWire A380X](https://github.com/flybywiresim/aircraft) to X-Plane 12.

## Install

**Windows 10 or later:** download the installer from [Releases](https://github.com/Vihaan2012-cmyk/A380-For-X-Plane-12-Based-on-The-FBW-A380/releases), pick your X-Plane folder and a texture size (1K to 8K). The installer also updates the aircraft, downloading only what changed.

**Mac and Linux (testing only):** run `bash mac.sh` or `bash linux.sh` from the latest release. They build the aircraft from this source. They are new, so please report what breaks.

## What's in it

- FlyByWire's A380 systems running in a native X-Plane plugin: electrical, hydraulics, fuel, bleed, flight controls, engines, APU and the ECAM.
- FlyByWire's cockpit displays, EFB and OIT, with failures, settings and loading.
- Ground crew, chocks, service vehicles and jet bridges.
- 18 liveries to pick from in the installer, each in 2K or 4K.

## Bugs

Open an [issue](https://github.com/Vihaan2012-cmyk/A380-For-X-Plane-12-Based-on-The-FBW-A380/issues) with your X-Plane `Log.txt`. It records the aircraft's state at every start.

## Credits

Based on the FlyByWire A380X by [FlyByWire Simulations](https://flybywiresim.com/). The cockpit displays run on the JS Library for X-Plane ([xp-ui/](xp-ui/)), written independently for this port.

Not made, endorsed or supported by FlyByWire Simulations, Microsoft, Laminar Research or Airbus.

## License

- **Code:** GPL v3 ([LICENSE](LICENSE)).
- **3D model and textures:** FlyByWire's, under [CC BY-NC 4.0](LICENSES/CC-BY-NC-4.0.txt): credit them and no commercial use. They ship with the releases, not this repository.
- **Liveries:** each belongs to its author, shipped with their permission.
- **Downloaded at install from their own sources:** SASL, the FTSiM+ A380RR_Sp sound pack and FlyByWire's FCU font. The .acf and airfoils are built from your own X-Plane 12's A330-300.

Third-party notices: [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt).
