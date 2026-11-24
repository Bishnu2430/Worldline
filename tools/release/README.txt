Worldline: a relativistic universe sandbox
==========================================

Drop black holes, neutron stars and planets into the real solar system and
watch general relativity play out. Every physics model is checked against
theory and observation.

Website: https://bishnu2430.github.io/Worldline/
Source:  https://github.com/Bishnu2430/Worldline

Free for non-commercial use: see LICENSE.txt.


Running it
----------

Double-click worldline.exe. There's nothing to install: it's one program,
with the solar system's data, the maps and the typeface inside it.

The first time, Windows may say "Windows protected your PC", because the
program isn't signed with a paid certificate. Press "More info", then
"Run anyway".

New to it? Press F1, or ? in the top bar, for the manual inside the app. It
starts at the beginning.


What it needs
-------------

- Windows 10 or 11, 64-bit.
- A graphics card that draws through DirectX 12 or Vulkan, as almost any
  from the last ten years does.

If it can't start its graphics, it says so in a message box. Updating the
graphics card's driver may help. Or, from a command prompt in this folder:

    worldline.exe --software

draws with the processor instead, slowly.

On a weaker computer, choose Display -> Quality -> Low in the app, or start
it with:

    worldline.exe --quality low


Some places to start
--------------------

From a command prompt in this folder (or from the Scenarios window, in the
top bar):

    worldline.exe --scenario gw150914       two black holes merge
    worldline.exe --scenario hulse-taylor   the pulsar, as a film
    worldline.exe --scenario eclipse        the eclipse of 12 August 2026


In this folder
--------------

worldline.exe   the program
LICENSE.txt     its license: free for non-commercial use (PolyForm
                Noncommercial 1.0.0); commercial use needs permission
CREDITS.md      where its data, maps and images come from, and their licenses
THIRD-PARTY-LICENSES.txt
                the libraries it's built with, and their licenses' texts
README.txt      this file
