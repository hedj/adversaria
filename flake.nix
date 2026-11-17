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

  # APE, the parser of Attempto Controlled English, at a commit after its
  # last release (6.7.180715, 2018): later commits keep it working with
  # current SWI-Prolog (the float_format flag is gone). LGPL-3.0.
  inputs.ape = {
    url = "git+https://github.com/Attempto/APE.git?rev=5f4d5354a45fb772763bf1a9543f508f15b28982";
    flake = false;
  };

  # The shell itself is nix/devshell.nix, shared with shell.nix, which gives
  # the same shell to `nix-shell` without experimental features.
  outputs = { self, nixpkgs, scasp, ape }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAll = f: nixpkgs.lib.genAttrs systems (s:
        f (import ./nix/devshell.nix { pkgs = nixpkgs.legacyPackages.${s}; scaspSrc = scasp; apeSrc = ape; }));
    in {
      packages = forAll (d: { scasp = d.scasp; ape = d.ape; });
      devShells = forAll (d: { default = d.shell; });
    };
}
