# The development shell for `nix-shell`, which needs no experimental
# features. It reads the pins from flake.lock, so it uses the same nixpkgs,
# s(CASP), APE and Clex as `nix develop`, and fetches them with Nix's stable
# built-ins. The shell is defined once, in nix/devshell.nix.
let
  lock = builtins.fromJSON (builtins.readFile ./flake.lock);
  nixpkgsLock = lock.nodes.nixpkgs.locked;
  scaspLock = lock.nodes.scasp.locked;
  apeLock = lock.nodes.ape.locked;
  clexLock = lock.nodes.clex.locked;
  nixpkgs = builtins.fetchTarball {
    url = nixpkgsLock.url;
    sha256 = nixpkgsLock.narHash;
  };
  scaspSrc = builtins.fetchGit {
    url = scaspLock.url;
    rev = scaspLock.rev;
    ref = scaspLock.ref;
  };
  apeSrc = builtins.fetchGit {
    url = apeLock.url;
    rev = apeLock.rev;
  };
  clexSrc = builtins.fetchGit {
    url = clexLock.url;
    rev = clexLock.rev;
  };
  # No user config or overlays, as with a flake: the same packages for all.
  pkgs = import nixpkgs { config = { }; overlays = [ ]; };
in
(import ./nix/devshell.nix { inherit pkgs scaspSrc apeSrc clexSrc; }).shell
