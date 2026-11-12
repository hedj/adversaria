{
  description = "Adversaria prototype: the tools docs/plan.md § Environment asks for";

  # A NixOS channel tarball, not github:NixOS/nixpkgs: some build hosts reach
  # channels.nixos.org but not GitHub's API. flake.lock pins the exact release.
  inputs.nixpkgs.url = "https://channels.nixos.org/nixos-26.05/nixexprs.tar.xz";

  # s(CASP) 1.1.4, the version the translator trial used; 2.x needs
  # SWI-Prolog 10. A flake input, so Nix fetches it and flake.lock pins it.
  inputs.scasp = {
    url = "git+https://github.com/SWI-Prolog/sCASP.git?ref=refs/tags/V1.1.4";
    flake = false;
  };

  outputs = { self, nixpkgs, scasp }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAll = f: nixpkgs.lib.genAttrs systems (s: f nixpkgs.legacyPackages.${s});
    in {
      packages = forAll (pkgs: {
        # Optional before stage B. Installed as plain Prolog source,
        # loaded with `swipl -p library=$SCASP_HOME/prolog`.
        scasp = pkgs.stdenvNoCC.mkDerivation {
          pname = "scasp";
          version = "1.1.4";
          src = scasp;
          dontBuild = true;   # we copy the source; its Makefile builds a binary we do not use
          installPhase = ''
            mkdir -p $out
            cp -r prolog LICENSE $out/
          '';
          meta.license = pkgs.lib.licenses.asl20;
        };
      });

      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = [
            pkgs.cargo        # the Rust workspace
            pkgs.rustc
            pkgs.clippy       # make test runs lints as errors
            pkgs.rustfmt
            pkgs.swi-prolog   # 9 or later, with incremental tabling
            pkgs.nodejs_22    # only the tests' oracle for RFC 8785 numbers
            pkgs.python3      # only tests/memex_in_a_pty.py, which plays a terminal
            pkgs.pandoc       # 3
            pkgs.git
            pkgs.gnumake
          ];
          SCASP_HOME = self.packages.${pkgs.stdenv.hostPlatform.system}.scasp;
        };
      });
    };
}
