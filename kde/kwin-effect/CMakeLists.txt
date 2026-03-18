cmake_minimum_required(VERSION 3.20)

project(quicker-kwin-effect LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_POSITION_INDEPENDENT_CODE ON)

find_package(Qt6 REQUIRED COMPONENTS Core DBus Gui)
find_package(Qt6 REQUIRED COMPONENTS Quick Widgets)
find_package(KF6Config REQUIRED)
find_package(KF6CoreAddons REQUIRED)
find_package(KF6WindowSystem REQUIRED)
find_package(PkgConfig REQUIRED)
find_package(Wayland REQUIRED COMPONENTS Server)
pkg_check_modules(EPOXY REQUIRED IMPORTED_TARGET epoxy)
pkg_check_modules(LIBDRM REQUIRED IMPORTED_TARGET libdrm)

find_library(KWIN_LIBRARY NAMES kwin REQUIRED PATHS /usr/lib /usr/lib64)

add_library(KWin::kwin SHARED IMPORTED)
set_target_properties(
    KWin::kwin
    PROPERTIES
        IMPORTED_LOCATION "${KWIN_LIBRARY}"
        INTERFACE_INCLUDE_DIRECTORIES "/usr/include/kwin"
        INTERFACE_LINK_LIBRARIES "Qt6::DBus;Qt6::Quick;Qt6::Widgets;Wayland::Server;KF6::ConfigCore;KF6::CoreAddons;KF6::WindowSystem;PkgConfig::EPOXY;PkgConfig::LIBDRM"
)

add_library(quickerradialeffect MODULE quickerradialeffect.cpp)

target_link_libraries(
    quickerradialeffect
    PRIVATE
        Qt6::Core
        Qt6::DBus
        Qt6::Gui
        KWin::kwin
)

target_compile_definitions(
    quickerradialeffect
    PRIVATE
        QT_NO_KEYWORDS
)

set_target_properties(
    quickerradialeffect
    PROPERTIES
        PREFIX ""
        LIBRARY_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/kwin/effects/plugins"
)
