# A380 for X-Plane 12 (Ported from FBW A380)

An unofficial, free, non-commercial port of the FlyByWire A380X to X-Plane 12.

Download the installer from [Releases](https://github.com/Vihaan2012-cmyk/A380-For-X-Plane-12-Based-on-The-FBW-A380/releases). It installs the aircraft, lets you pick 1K, 2K, 4K or 8K textures, and has a Liveries page where each livery comes in 2K or 4K.

## Displays

The cockpit displays are FlyByWire's A380X instruments, running in a browser view inside X-Plane. They run on our own generic JavaScript library, the **JS Library for X-Plane** ([xp-ui/](xp-ui/)), written independently for this port and usable by any aircraft:

- `xplane-js/data`: datarefs, commands, plugin variables, units and the instrument host.
- `xplane-js/ui`: reactive values, an event bus, components with a JSX renderer, and maths, formatting and navigation helpers.

FlyByWire's instrument code was moved onto this library and adapted to match its API, so the displays do not use Microsoft's MSFS SDK or avionics framework. Every release is checked against them before it is packaged.

## Credits

Based on the FlyByWire A380X by [FlyByWire Simulations](https://flybywiresim.com/).
Original source code: [flybywiresim/aircraft](https://github.com/flybywiresim/aircraft).

This port is not made, endorsed or supported by FlyByWire Simulations, Microsoft, Laminar Research or Airbus.

## License

- **Source code:** GNU General Public License v3, see [LICENSE](LICENSE). This covers the systems plugin, the instrument pages, the JS Library for X-Plane, the installer and the rest of the source code. You can redistribute and modify them under the terms of the GPL, version 3 or any later version. They are distributed WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.
- **3D model and textures:** [Creative Commons Attribution-NonCommercial 4.0 International](https://creativecommons.org/licenses/by-nc/4.0/), see [LICENSES/CC-BY-NC-4.0.txt](LICENSES/CC-BY-NC-4.0.txt). The model is FlyByWire's, converted to X-Plane for this port, and stays under their license: credit FlyByWire Simulations and do not use it commercially. The models are not in this repository; they ship with the releases.
- **Liveries:** each livery belongs to its author and is shipped with their permission.
- **Not included, downloaded during installation from their own sources:** SASL (1-sim), FTSiM+'s A380RR_Sp sound pack and FlyByWire's FCU font. The .acf and wing airfoils are built from your own X-Plane 12's Airbus A330-300.

Third-party components and their licenses: [THIRD_PARTY_NOTICES.txt](THIRD_PARTY_NOTICES.txt).
