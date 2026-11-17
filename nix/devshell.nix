# The development shell, shared by flake.nix (nix develop) and shell.nix
# (nix-shell, which needs no experimental features). Both pass the same
# nixpkgs, s(CASP) source and APE source, pinned in flake.lock.
{ pkgs, scaspSrc, apeSrc }:
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
  # APE, the parser of Attempto Controlled English, with its small Clex
  # lexicon. Its grammar is written in .fit files, compiled here to .plp
  # files as its Makefile's plp target does; the memex loads the Prolog
  # source with `swipl -p ape=$APE_HOME/prolog`.
  ape = pkgs.stdenvNoCC.mkDerivation {
    pname = "ape";
    version = "6.7-unstable-2024-04-21";
    src = apeSrc;
    nativeBuildInputs = [ pkgs.swi-prolog ];
    buildPhase = ''
      runHook preBuild
      swipl -O -F none -g "working_directory(_, 'prolog/parser'), [fit_to_plp], halt." -t halt
      runHook postBuild
    '';
    installPhase = ''
      mkdir -p $out
      cp -r prolog LICENSE.txt $out/
    '';
    meta.license = pkgs.lib.licenses.lgpl3Only;
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
      pkgs.python3      # only tests/memex_in_a_pty.py, which plays a terminal
      pkgs.pandoc       # 3
      pkgs.curl         # the source check (Crossref) and the wikidata tool
      pkgs.git
      pkgs.gnumake
    ];
    SCASP_HOME = scasp;
    APE_HOME = ape;
  };
}
