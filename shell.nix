{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  shellHook = ''
    # INFO winit::platform_impl::platform::x11::window: Guessed window scale factor: 1.1666666666666667
    export WINIT_X11_SCALE_FACTOR=1

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
