**A relativistic universe sandbox.** Drop black holes, neutron stars and planets into the real solar system and watch general relativity play out. Every physics model is checked against theory and observation: see the [website](https://bishnu2430.github.io/Worldline/) and the [validation plots](https://github.com/Bishnu2430/Worldline/blob/main/docs/validation/README.md).

### Download and run it

1. Download the zip below, `Worldline-…-windows-x64.zip`, and unzip it.
2. Double-click `worldline.exe`. There's nothing to install.
3. The first time, Windows may say *Windows protected your PC*, because the program isn't signed with a paid certificate. Press **More info**, then **Run anyway**.

It needs Windows 10 or 11 (64-bit) and a graphics card that draws through DirectX 12 or Vulkan. It's free for non-commercial use, under the [PolyForm Noncommercial License 1.0.0](https://github.com/Bishnu2430/Worldline/blob/main/LICENSE). Press **F1** in the app for the manual. `README.txt` in the zip has the rest.

### Checked before it was published

- **Nothing to install:** the program's own header was read. It's a windowed 64-bit program that loads nothing but Windows' own DLLs, with the C runtime built in, so no Visual C++ redistributable is needed.
- **It starts on a clean machine:** a fresh Windows virtual machine with only this zip (no source and no build tools) unzipped it, started it, drew the solar system and ran its physics.

The build's log has both, in this release's **Release** workflow run.
