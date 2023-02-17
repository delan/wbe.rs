{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  shellHook = ''
    # INFO winit::platform_impl::platform::x11::window: Guessed window scale factor: 1.1666666666666667
    # export WINIT_X11_SCALE_FACTOR=1

    # Times New Roman
    export WBE_FONT_PATH=${pkgs.corefonts.outPath}/share/fonts/truetype/times.ttf
    export WBE_FONT_PATH_B=${pkgs.corefonts.outPath}/share/fonts/truetype/timesbd.ttf
    export WBE_FONT_PATH_I=${pkgs.corefonts.outPath}/share/fonts/truetype/timesi.ttf
    export WBE_FONT_PATH_BI=${pkgs.corefonts.outPath}/share/fonts/truetype/timesbi.ttf

    export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath [
      pkgs.xorg.libX11
      pkgs.xorg.libXcursor
      pkgs.xorg.libXrandr
      pkgs.xorg.libXi
      pkgs.libglvnd
    ]}
  '';

  buildInputs = [
      pkgs.xorg.libX11
      pkgs.xorg.libXcursor
      pkgs.xorg.libXrandr
      pkgs.xorg.libXi
      pkgs.libglvnd
  ];
}
