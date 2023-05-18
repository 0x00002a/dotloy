# Dotoy

## Introduction

This is a tool for my own personal use for deploying my dotfiles


## Usage

Config file:

### Template expansion

Preprocessing is done on all files that end with `.in`. Variables other than the default
can be defined by setting the `variables` in the config for that file.

### Hard/Soft linking

Linking can be done by setting the `to` variable on the path. Files are hard-linked by default
while directories are soft-linked. This can be changed by setting `link_type` to `soft` or `hard`
(`hard` is invalid for directories).

### Hardlinks

Create a file with the extension `
