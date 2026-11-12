{
  description = "Fast native WhatsApp client";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    # rust-toolchain.toml pins the compiler so local builds and CI agree.
    # This reads that file rather than restating the version here.
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      ...
    }:
    let
      systems = [
        "aarch64-linux"
        "x86_64-linux"
        # Nixpkgs unstable no longer supports x86_64-darwin.
        "aarch64-darwin"
      ];
      forAllSystems =
        f:
        nixpkgs.lib.genAttrs systems (
          system:
          f (
            import nixpkgs {
              inherit system;
              overlays = [ (import rust-overlay) ];
            }
          )
        );
    in
    {
      devShells = forAllSystems (
        pkgs:
        let
          inherit (pkgs.stdenv.hostPlatform) isLinux;
        in
        {
          default = pkgs.mkShell (
            {
              packages =
                with pkgs;
                [
                  (rust-bin.fromRustupToolchainFile ./rust-toolchain.toml)
                  rust-analyzer
                  pkg-config
                  cmake
                  perl
                ]
                # On macOS the app links the system frameworks instead.
                ++ lib.optionals isLinux [
                  alsa-lib
                  pipewire
                  # The PipeWire output's bindings are made by bindgen,
                  # which needs libclang.
                  rustPlatform.bindgenHook
                  libxkbcommon
                  wayland
                  libGL
                  libx11
                  libxcursor
                  libxi
                  libxrandr
                ];
              ZAPFASTIE_TEST_RTL_FONT = "${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans.ttf";
            }
            // pkgs.lib.optionalAttrs isLinux {
              # The GUI dlopens its Wayland, X11 and GL libraries at run time.
              LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (
                with pkgs;
                [
                  libxkbcommon
                  wayland
                  libGL
                  libx11
                  libxcursor
                  libxi
                  libxrandr
                ]
              );
            }
          );
        }
      );

      packages = forAllSystems (
        pkgs:
        let
          inherit (pkgs.stdenv.hostPlatform) isLinux isDarwin;
          toolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
          rustPlatform = pkgs.makeRustPlatform {
            cargo = toolchain;
            rustc = toolchain;
          };
          runtimeLibs = with pkgs; [
            libxkbcommon
            wayland
            libGL
            libx11
            libxcursor
            libxi
            libxrandr
          ];
          zapfastie = rustPlatform.buildRustPackage rec {
            pname = "zapfastie";
            version = (pkgs.lib.importTOML ./Cargo.toml).package.version;
            src = self;

            # Import registry dependencies directly from Cargo.lock so ordinary
            # lock-file updates do not require refreshing a vendor hash. Git
            # dependencies remain pinned to their exact locked revisions.
            cargoLock = {
              lockFile = ./Cargo.lock;
              allowBuiltinFetchGit = true;
            };

            nativeBuildInputs =
              with pkgs;
              [
                pkg-config
                cmake
                perl
                makeWrapper
              ]
              # bindgen, for the PipeWire output's bindings, needs libclang.
              ++ lib.optionals isLinux [
                removeReferencesTo
                rustPlatform.bindgenHook
              ]
              # Sandboxed stand-ins for iconutil and codesign in bundle.sh.
              ++ lib.optionals isDarwin [
                icnsify
                rcodesign
              ];
            # On macOS the app links the system frameworks instead.
            buildInputs = pkgs.lib.optionals isLinux (
              with pkgs;
              [
                alsa-lib
                pipewire
                libGL
                libx11
              ]
            );
            ZAPFASTIE_TEST_RTL_FONT = "${pkgs.dejavu_fonts}/share/fonts/truetype/DejaVuSans.ttf";

            # The vendored OpenSSL (through rusqlite's bundled SQLCipher) embeds
            # its build line, compiler path included, in the binary. The app
            # never runs it, but the text alone would keep the whole compiler
            # (about 280 MiB) in the runtime closure. Fail the build if a
            # reference survives.
            disallowedReferences = pkgs.lib.optionals isLinux [ pkgs.stdenv.cc ];

            postFixup =
              # The GUI dlopens its Wayland, X11 and GL libraries at run time.
              pkgs.lib.optionalString isLinux ''
                find $out/bin -type f -exec remove-references-to -t ${pkgs.stdenv.cc} {} +
                wrapProgram $out/bin/zapfastie \
                  --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath runtimeLibs}
              ''
              # Sign after the strip hook, which would invalidate the signature.
              # The entitlement allows microphone access for voice messages.
              + pkgs.lib.optionalString isDarwin ''
                rcodesign sign \
                  --entitlements-xml-file packaging/macos/entitlements.plist \
                  $out/Applications/ZapFastie.app
              '';

            postInstall =
              pkgs.lib.optionalString isLinux ''
                install -Dm644 packaging/applications/zapfastie.desktop \
                  $out/share/applications/zapfastie.desktop
                install -Dm644 packaging/icons/zapfastie.svg \
                  $out/share/icons/hicolor/scalable/apps/zapfastie.svg
                install -Dm644 contrib/omarchy/zapfastie.json.tpl \
                  $out/share/zapfastie/omarchy/zapfastie.json.tpl
                install -Dm755 contrib/omarchy/zapfastie-theme \
                  $out/share/zapfastie/omarchy/zapfastie-theme
              ''
              # Mirrors packaging/macos/bundle.sh: ZapFastie.app around the built
              # binary, with the version stripped of any prerelease suffix.
              + pkgs.lib.optionalString isDarwin ''
                app=$out/Applications/ZapFastie.app
                mkdir -p $app/Contents/MacOS $app/Contents/Resources
                mv $out/bin/zapfastie $app/Contents/MacOS/zapfastie
                chmod 755 $app/Contents/MacOS/zapfastie
                # The command runs the bundled executable, so macOS applies the
                # bundle's Info.plist and entitlements, and start at login
                # records the bundled path.
                makeWrapper $app/Contents/MacOS/zapfastie $out/bin/zapfastie
                sed "s/__VERSION__/''${version%%-*}/g" packaging/macos/Info.plist \
                  > $app/Contents/Info.plist
                icnsify packaging/macos/icon-1024.png \
                  --output $app/Contents/Resources/zapfastie.icns
              '';

            meta = {
              description = "Fast native WhatsApp client";
              homepage = "https://zapfast.rocks";
              license = with pkgs.lib.licenses; [ mit gpl2Only ];
              mainProgram = "zapfastie";
              # The flake publishes only aarch64-darwin. Older nixpkgs, for
              # example through follows, still list x86_64-darwin.
              platforms = pkgs.lib.platforms.linux ++ [ "aarch64-darwin" ];
            };
          };
        in
        {
          default = zapfastie;
          inherit zapfastie;
        }
      );

      homeManagerModules = {
        default = self.homeManagerModules.zapfastie;
        zapfastie = import ./nix/home-manager.nix self;
      };

      formatter = forAllSystems (pkgs: pkgs.nixfmt-tree);
    };
}
