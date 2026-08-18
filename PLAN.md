
# Aim of the project


- Build a checklist server with api endpoints to create, read, update and delete checklists.
- The server should be able to handle multiple checklists and each checklist can have multiple items.
- A checklist has a title and a description, while each item a step description.
- The server don't have user concepts, everyone can create and modify  checklists, modified checklists will be stored as a new checklist.
- checklist has a creation date (createdAt), and is ready-only in the sense that it cannot be updated/deleted, only new checklists can be created from existing ones.
- Creator ip address should be stored for each checklist and item, but not exposed in the api endpoints.
- Standard RESTful api endpoints should be provided for the checklists, the standard CRUD operations should be supported, and the endpoints should be documented using OpenAPI specification.
- One can search for checklists by title, description, creation date, and item step description.
- Standard process should be search to locate a checklist, modify it, and create a new checklist from it.
- You can get a random checklist (already exist) from the server.
- A checklist should be packed as a single UTF-8 encoded string, and the server should be able to unpack it back to a checklist object.
- The server should be able to export a checklist as a JSON file, and import a checklist from a JSON file, also.
- The format of the JSON file should be documented in the OpenAPI specification.
- The format of the packed checklist string should be documented in the OpenAPI specification.
- checklist core should be implemented in a separate crate, and the server should be implemented in another crate, which uses the core crate.
- The server should be implemented in Rust, and the core crate should be implemented in Rust as well.
- These two crates should be published to crates.io, and the server crate should depend on the core crate.
- The two crates should be hosted on GitHub, there should be actions to build and publish them.
- Version should be managed using git tags, and the version should be updated in the Cargo.toml files of both crates.
- The server should be able to run as a standalone binary.
- The server should be able to be configured using a configuration file, and the configuration file format should be documented.
- All information about the checklists should be stored in a database file, perferably using SQLite, and the database file format should be documented.
- All information about the checklists should be open and accessible to everyone, and the database file should be downloadable from the server.
- Checklist core should be implemented in a way that it can be used in other programming languages, and the core crate should provide a C API for this purpose.
- Checklist core can be used with sqlite file downloaded from the server, and can create new checklists, and can read existing checklists from the sqlite file.