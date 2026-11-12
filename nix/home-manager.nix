self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  cfg = config.programs.zapfastie;
  json = pkgs.formats.json { };
  # paths.rs: ProjectDirs::from("me", "paolino", "zapfastie").
  configDir =
    if pkgs.stdenv.hostPlatform.isDarwin then
      "${config.home.homeDirectory}/Library/Application Support/me.paolino.zapfast"
    else
      "${config.xdg.configHome}/zapfastie";
  declared = json.generate "zapfastie-settings.json" cfg.settings;
  jq = lib.getExe pkgs.jq;
  readSecret = key: path: ''
    if [[ -r ${lib.escapeShellArg path} ]]; then
      zapfastSecrets+=(--rawfile ${lib.escapeShellArg key} ${lib.escapeShellArg path})
    else
      warnEcho ${lib.escapeShellArg "ZapFastie: cannot read ${path}, keeping the current ${key}"}
    fi
  '';
in
{
  options.programs.zapfastie = {
    enable = lib.mkEnableOption "ZapFastie, a native WhatsApp client";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "zapfastie.packages.\${pkgs.stdenv.hostPlatform.system}.default";
      description = "The ZapFastie package to install.";
    };

    settings = lib.mkOption {
      type = lib.types.attrsOf json.type;
      default = { };
      example = lib.literalExpression ''
        {
          check_for_updates = false;
          message_sound = "none";
          theme = "dark";
        }
      '';
      description = ''
        Keys merged into ZapFastie's `settings.json` on every activation. Keys
        left out keep whatever the app last saved, so window geometry, recent
        emoji and the lock verifiers survive. A key declared here is reset on
        the next activation if it is changed in the app. Per-account preferences
        such as read receipts live in each account's own file and cannot be
        declared here. Restart ZapFastie after
        activation: it reads the file at start, and its next save writes back
        the values it holds in memory.

        Values are copied to the Nix store, which every local user can read.
        Give secrets, such as proxy credentials or `giphy_key`, through
        {option}`programs.zapfastie.secretFiles` instead.
      '';
    };

    secretFiles = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      example = lib.literalExpression ''
        {
          proxy = config.sops.secrets.zapfastie-proxy.path;
        }
      '';
      description = ''
        Keys of `settings.json` whose string values are read at activation from
        these files, for example secrets decrypted by sops-nix or agenix, so
        they never enter the Nix store. Give each path as a string: a Nix path
        literal would be copied to the store. A trailing newline is dropped. A
        file that cannot be read leaves that key as it is, with a warning; a
        secret manager that decrypts after activation fills it on the next
        activation.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    assertions = [
      {
        assertion = builtins.all (key: !(cfg.settings ? ${key})) (builtins.attrNames cfg.secretFiles);
        message = "programs.zapfastie: a key cannot be in both settings and secretFiles.";
      }
    ];

    home.packages = [ cfg.package ];

    # settings.json cannot be a store symlink: the app replaces it whole on
    # every change and keeps its own state in it.
    home.activation.zapfastSettings = lib.mkIf (cfg.settings != { } || cfg.secretFiles != { }) (
      lib.hm.dag.entryAfter [ "writeBoundary" ] ''
        zapfastDir=${lib.escapeShellArg configDir}
        zapfastFile="$zapfastDir/settings.json"
        run mkdir -p -m 700 "$zapfastDir"
        if [[ ! -e "$zapfastFile" ]]; then
          # A file without a version is migrated as one from older releases,
          # which would replace an explicit beige or black wallpaper.
          zapfastBase='{"version":1}'
        elif zapfastBase=$(${jq} -ce objects "$zapfastFile" 2>/dev/null); then
          :
        else
          warnEcho "ZapFastie: $zapfastFile is not a JSON object, leaving it unchanged"
          zapfastBase=
        fi
        if [[ -z "$zapfastBase" ]]; then
          :
        elif [[ -v DRY_RUN ]]; then
          echo "Would merge ${declared} into $zapfastFile"
        else
          zapfastSecrets=()
          ${lib.concatStrings (lib.mapAttrsToList readSecret cfg.secretFiles)}
          zapfastTmp=$(mktemp "$zapfastDir/.settings.json.XXXXXX")
          ${jq} --slurpfile __zapfastDeclared ${declared} "''${zapfastSecrets[@]}" \
            '. * $__zapfastDeclared[0] * ($ARGS.named | del(.__zapfastDeclared) | map_values(rtrimstr("\n")))' \
            <<< "$zapfastBase" > "$zapfastTmp"
          chmod 600 "$zapfastTmp"
          mv "$zapfastTmp" "$zapfastFile"
        fi
        unset zapfastDir zapfastFile zapfastBase zapfastSecrets zapfastTmp
      ''
    );
  };
}
