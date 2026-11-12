# The development shell, shared by flake.nix (nix develop) and shell.nix
# (nix-shell, which needs no experimental features). Both pass the same
# nixpkgs and s(CASP) source, pinned in flake.lock.
{ pkgs, scaspSrc }:
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
in {
  inherit scasp;
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
  };
}
