// Empty C++ translation unit. build.rs compiles this with the C++ driver so
// the C++ runtime is linked next to the ZopfliPNG objects. Those objects are
// C and C++ mixed, and the C sources must not be compiled as C++.
extern "C" void crunch_cxx_link_anchor() {}
