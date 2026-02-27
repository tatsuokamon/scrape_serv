cmake_minimum_required(VERSION 3.20)
project(MyApp LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

find_package(nlohmann_json CONFIG REQUIRED)
find_package(unofficial-gumbo CONFIG REQUIRED)
find_package(date REQUIRED)

add_library(cpp
	STATIC
    src/find_meta.cpp
    src/find_max_idx.cpp
    src/lib.cpp
    src/find_detail.cpp
    src/update_tag.cpp
)

target_link_libraries(cpp PRIVATE nlohmann_json::nlohmann_json unofficial::gumbo::gumbo date::date)
target_include_directories(cpp PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/include)

