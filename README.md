# Dotoy

## Introduction

This is a tool for my own personal use for deploying my dotfiles


## Usage

Config file is specified in yaml. If not specified it will look for it at `<cwd>/dotloy.yaml`

### Template expansion

Preprocessing is done on all files that end with `.in`. Variables other than the default
can be defined by setting the `variables` in the config for that file.

The template syntax is similar to handlebars, that is `{{ var }}` will expand
to whatever `var` is set to. Namespaces are done with `.`.

#### Toplevel variables

- `cwd`: Directory in which the config file resides
- `config.`: Namespace for variables defined at the toplevel of the config
- `target.`: Namespace for variables defined on each target

### Hard/Soft linking

Linking can be done by setting the `to` variable on the path. Files are hard-linked by default
while directories are soft-linked. This can be changed by setting `link_type` to `soft` or `hard`
(`hard` is invalid for directories).
