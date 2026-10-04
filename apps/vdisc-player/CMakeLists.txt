cmake_minimum_required(VERSION 3.21)

project(vdisc_player VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(Qt6 6.8 REQUIRED COMPONENTS Quick Quick3D)
qt_standard_project_setup(REQUIRES 6.8)

qt_add_executable(vdisc-player main.cpp)

file(GLOB greybox_meshes RELATIVE "${CMAKE_CURRENT_SOURCE_DIR}" CONFIGURE_DEPENDS
    "${CMAKE_CURRENT_SOURCE_DIR}/generated/phase03-greybox/meshes/*.mesh"
)

qt_add_qml_module(vdisc-player
    URI VdiscPlayer
    VERSION 1.0
    QML_FILES
        qml/Main.qml
        generated/phase03-greybox/Phase03_greybox.qml
    RESOURCES ${greybox_meshes}
)

target_link_libraries(vdisc-player PRIVATE Qt6::Quick Qt6::Quick3D)
