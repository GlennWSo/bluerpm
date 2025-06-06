{
  inputs = {
    nixpkgs.url = "github:NixOs/nixpkgs";
    flake-utils.url = "github:numtide/flake-utils";
    crane = {
      url = "github:ipetkov/crane";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-filter.url = "github:numtide/nix-filter";
  };

  outputs = {
    flake-utils,
    nixpkgs,
    rust-overlay,
    crane,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (localSystem: let
      overlays = [(import rust-overlay)];
      pkgs = import nixpkgs {inherit overlays localSystem;};
      rust = pkgs.rust-bin.stable.latest.default.override {
        targets = ["thumbv7em-none-eabihf"];
      };
      craneLib = (crane.mkLib pkgs).overrideToolchain rust;

      fs = pkgs.lib.fileset;
      files = fs.unions [
        ./.cargo
        # ./workspace-hack
        (fs.fileFilter (file: file.hasExt "toml") ./.)
        (fs.fileFilter (file: file.name == "dummy.rs") ./.)
        (fs.fileFilter (file: file.name == "Cargo.lock") ./.)
      ];
      srcFiles = fs.unions [
        files
        (fs.fileFilter (file: file.hasExt "rs") ./.)
        (fs.fileFilter (file: file.name == "memory.x") ./.)
      ];
      src = fs.toSource {
        root = ./.;
        fileset = srcFiles;
      };
      dummySrc = craneLib.mkDummySrc {
        src = src;
        extraDummyScript = ''
        '';
      };
      cargoArtifacts = craneLib.buildDepsOnly {
        inherit src dummySrc;
        pname = "deps";
        version = "0.1.0";
        doCheck = false;
        cargoExtraArgs = "--target thumbv7em-none-eabihf";
      };
      mkCrate = toml: (
        let
          info = craneLib.crateNameFromCargoToml {
            cargoToml = toml;
          };
          pname = info.pname;
          version = info.version;
        in
          craneLib.buildPackage {
            inherit src cargoArtifacts pname version;
            doCheck = false;
            cargoExtraArgs = "--target thumbv7em-none-eabihf -p ${pname}";
          }
      );
      rpmsensor = mkCrate ./rpmsensor/Cargo.toml;
      blinky = mkCrate ./blinky/Cargo.toml;

      udev_hint = ''
        "hint: make sure the microbit is connected and have mod 666 to enable flashing
        this can be achived with sudo chmod or udev settings:
          SUBSYSTEM=="usb", ATTR{idVendor}=="0d28", ATTR{idProduct}=="0204", MODE:="666""
      '';
      embedder = fw: (pkgs.writeShellScript "embed-" ''
        ${pkgs.probe-rs}/bin/probe-rs run ${fw}/bin/${fw.pname} --chip nRF52833_xxAA || echo ${udev_hint}
      '');
      embedApp = fw: {
        type = "app";
        program = "${embedder fw}";
      };
    in {
      devShells.default = craneLib.devShell {
        name = "embeded-rs";
        inputsFrom = [rpmsensor];
        DIRENV_LOG_FORMAT = "";
        DEFMT_LOG = "info";
        shellHook = "
        ";
        packages = with pkgs; [
          probe-rs
          rust-analyzer
          cargo-binutils
          minicom
          usbutils
          cargo-hakari
          python312Packages.python-lsp-server
        ];
      };
      apps = {
        default = embedApp rpmsensor;
      };

      dbg = {
        dummySrc = dummySrc;
      };
      packages = {
        inherit rpmsensor blinky cargoArtifacts;
        default = rpmsensor;
      };
    });
}
