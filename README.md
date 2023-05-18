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
- `xdg.`: Namespace for xdg standard paths
  - `home`: Home directory
  - `config`: Top level config dir, same on linux as `local.config` but on windows it uses `/Roaming` rather than `/Local`
  - `local.`: Namespace for local xdg paths
    - `config`: Config path, only differs on windows

### Hard/Soft linking

Linking can be done by setting the `to` variable on the path. Files are hard-linked by default
while directories are soft-linked. This can be changed by setting `link_type` to `soft` or `hard`
(`hard` is invalid for directories).


## Example usage

Say I have a config file for my zsh and I want to break it up into different
files, one for aliases, one for env variables, etc. I don't want to have to deploy all of these
files to my zsh root so instead I source them from zshrc. In order to make this not dependent
of the location of the dotfiles dir I use templates:

config file:

```yaml
variables:
    loaddir: '{{ cwd }}'
targets:
    - from: zshrc.in
      to: '{{ xdg.home }}/.zshrc'
```

zshrc.in:

```zsh
source {{ config.loaddir }}/aliases
source {{ config.loaddir }}/env
```

Then we can just run `dotloy deploy` in the directory and it will expand the
templates in `zshrc.in` and then copy the result to whatever the home directory
is on the current os.
