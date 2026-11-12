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

  # The shell itself is nix/devshell.nix, shared with shell.nix, which gives
  # the same shell to `nix-shell` without experimental features.
  outputs = { self, nixpkgs, scasp }:
    let
      systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
      forAll = f: nixpkgs.lib.genAttrs systems (s:
        f (import ./nix/devshell.nix { pkgs = nixpkgs.legacyPackages.${s}; scaspSrc = scasp; }));
    in {
      packages = forAll (d: { scasp = d.scasp; });
      devShells = forAll (d: { default = d.shell; });
    };
}
