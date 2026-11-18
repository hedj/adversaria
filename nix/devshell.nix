# The development shell, shared by flake.nix (nix develop) and shell.nix
# (nix-shell, which needs no experimental features). Both pass the same
# nixpkgs, and the s(CASP), APE and Clex sources, pinned in flake.lock.
{ pkgs, scaspSrc, apeSrc, clexSrc }:
let
  # s(CASP) 1.1.4, the version the translator trial used; 2.x needs
  # SWI-Prolog 10. Optional before stage B. Installed as plain Prolog
  # source, loaded with `swipl -p library=$SCASP_HOME/prolog`.
  scasp = pkgs.stdenvNoCC.mkDerivation {
    pname = "scasp";
    version = "1.1.4";
    src = scaspSrc;
    dontBuild = true;   # we copy the source; its Makefile builds a binary we do not use
    installPhase = ''
      mkdir -p $out
      cp -r prolog LICENSE $out/
    '';
    meta.license = pkgs.lib.licenses.asl20;
  };
  # APE, the parser of Attempto Controlled English, with the large Clex
  # lexicon in place of its small one, as Clex's README says. Its grammar
  # is written in .fit files, compiled here to .plp files as its
  # Makefile's plp target does; the memex loads the Prolog source with
  # `swipl -p library=$APE_HOME/prolog`.
  ape = pkgs.stdenvNoCC.mkDerivation {
    pname = "ape";
    version = "6.7-unstable-2024-04-21";
    src = apeSrc;
    nativeBuildInputs = [ pkgs.swi-prolog ];
    buildPhase = ''
      runHook preBuild
      cp ${clexSrc}/clex_lexicon.pl prolog/lexicon/clex_lexicon.pl
      swipl -O -F none -g "working_directory(_, 'prolog/parser'), [fit_to_plp], halt." -t halt
      runHook postBuild
    '';
    installPhase = ''
      mkdir -p $out
      cp -r prolog LICENSE.txt $out/
      cp ${clexSrc}/LICENSE $out/CLEX-LICENSE
    '';
    meta.license = [ pkgs.lib.licenses.lgpl3Only pkgs.lib.licenses.gpl3Only ];
  };
in {
  inherit scasp ape;
  shell = pkgs.mkShell {
    packages = [
      pkgs.cargo        # the Rust workspace
      pkgs.rustc
      pkgs.clippy       # make test runs lints as errors
      pkgs.rustfmt
      pkgs.swi-prolog   # 9 or later, with incremental tabling
      pkgs.nodejs_22    # only the tests' oracle for RFC 8785 numbers
      pkgs.python3      # tests/memex_in_a_window.py, which types into the window
      pkgs.pandoc       # 3
      pkgs.curl         # the source check (Crossref) and the wikidata tool
      pkgs.git
      pkgs.gnumake
      # The window's test: an X server with no screen, a keyboard to type
      # with, and a headless Wayland compositor with its own.
      pkgs.xvfb
      pkgs.xdotool
      pkgs.sway
      pkgs.wtype
      pkgs.grim
    ];
    SCASP_HOME = scasp;
    APE_HOME = ape;
    # The window (crates/memex/src/gui) loads these when it runs: the
    # Vulkan and GL loaders, the keyboard map, Wayland and X11.
    LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [
      vulkan-loader libglvnd libxkbcommon wayland
      libx11 libxcursor libxi libxrandr
    ]);
    # Mesa's drivers, unless the user's own are set: a program built by Nix
    # does not find a system's drivers. (NVIDIA's own driver, outside NixOS,
    # needs nixGL.)
    shellHook = ''
      export __EGL_VENDOR_LIBRARY_DIRS=''${__EGL_VENDOR_LIBRARY_DIRS:-${pkgs.mesa}/share/glvnd/egl_vendor.d}
      export LIBGL_DRIVERS_PATH=''${LIBGL_DRIVERS_PATH:-${pkgs.mesa}/lib/dri}
      export VK_ADD_DRIVER_FILES=''${VK_ADD_DRIVER_FILES:-$(ls ${pkgs.mesa}/share/vulkan/icd.d/*.json | tr '\n' ':')}
    '';
  };
}
