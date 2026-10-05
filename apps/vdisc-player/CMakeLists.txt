cmake_minimum_required(VERSION 3.24)

project(vdisc_player VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 6.8 REQUIRED COMPONENTS Quick Quick3D Qml QuickTest)
find_package(ALSA REQUIRED)
qt_standard_project_setup(REQUIRES 6.8)

find_package(CxxQt QUIET)
if(NOT CxxQt_FOUND)
    include(FetchContent)
    FetchContent_Declare(
        CxxQt
        GIT_REPOSITORY https://github.com/kdab/cxx-qt-cmake.git
        GIT_TAG 0.10.0
    )
    FetchContent_MakeAvailable(CxxQt)
endif()

cxx_qt_import_crate(
    MANIFEST_PATH ../../crates/vdisc-qml-bridge/Cargo.toml
    CRATES vdisc_qml_bridge
    FEATURES qml
    LOCKED
    QT_MODULES Qt::Core Qt::Gui Qt::Qml
)

cxx_qt_import_qml_module(vdisc_appliance_qml_module
    URI "VdiscAppliance"
    SOURCE_CRATE vdisc_qml_bridge
)

qt_add_executable(vdisc-player main.cpp)

file(GLOB greybox_meshes RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}" CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/generated/phase03-greybox/meshes/*.mesh"
)

qt_add_qml_module(vdisc-player
    URI VdiscPlayer
    VERSION 1.0
    QML_FILES
        qml/Main.qml
        qml/DiscDeck.qml
        qml/DiscPlacement.js
        generated/phase03-greybox/Phase03_greybox.qml
    RESOURCES ${greybox_meshes}
)

target_link_libraries(vdisc-player PRIVATE Qt6::Quick Qt6::Quick3D vdisc_appliance_qml_module ALSA::ALSA)

qt_add_executable(vdisc-appliance-tests tests/quick_test_main.cpp)
target_compile_definitions(vdisc-appliance-tests PRIVATE
    QUICK_TEST_SOURCE_DIR="${CMAKE_CURRENT_SOURCE_DIR}/tests"
)
target_link_libraries(vdisc-appliance-tests PRIVATE
    Qt6::QuickTest Qt6::Qml vdisc_appliance_qml_module ALSA::ALSA
)

# The core reader launches these sandboxed validators beside the executable.
find_program(CARGO_EXECUTABLE cargo REQUIRED)
get_filename_component(VDISC_WORKSPACE_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." REALPATH)
set(VDISC_WORKER_TARGET_DIR "${CMAKE_CURRENT_BINARY_DIR}/cargo/workers")
add_custom_target(vdisc-workers
    COMMAND ${CMAKE_COMMAND} -E env
        "CARGO_TARGET_DIR=${VDISC_WORKER_TARGET_DIR}"
        "CARGO_PROFILE_DEV_DEBUG=0"
        ${CARGO_EXECUTABLE} build --locked
            --manifest-path "${VDISC_WORKSPACE_ROOT}/Cargo.toml"
            -p vdisc-core --bins
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
        "${VDISC_WORKER_TARGET_DIR}/debug/vdisc-media-check"
        "${CMAKE_CURRENT_BINARY_DIR}/vdisc-media-check"
    COMMAND ${CMAKE_COMMAND} -E copy_if_different
        "${VDISC_WORKER_TARGET_DIR}/debug/vdisc-image-check"
        "${CMAKE_CURRENT_BINARY_DIR}/vdisc-image-check"
    VERBATIM
)
add_dependencies(vdisc-player vdisc-workers)
add_dependencies(vdisc-appliance-tests vdisc-workers)
