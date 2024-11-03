# SFTP-FastDL

A [fastdl](https://developer.valvesoftware.com/wiki/FastDL) helper, designed to run as a docker container. Built with [AMP](https://cubecoders.com/AMP) in mind, this serves game files via HTTP by streaming them through SFTP to the client.

Also handles files requested with bzip encoding by on-the-fly stream compressing them as it reads from the SFTP server.

## Environment Variables

| Name | Type | Description | Default |
| ---- | ---- | ---- | ---- |
| `PORT` | u16 | port the HTTP server binds to | 3000 |
| `SFTP_HOST` | string | hostname of the SFTP server | |
| `SFTP_PORT` | u16 | port of the SFTP server | |
| `SFTP_USERNAME` | string | username used to authenticate to the sftp server | |
| `SFTP_PASSWORD` | string | password for the user used during authentication to the sftp server | |
| `SFTP_PATH` | string | base path files will be served from | |

## Docker Image

Published to Github Container registry at https://github.com/rob0rt/sftp-fastdl/pkgs/container/sftp-fastdl

Image name: `ghcr.io/rob0rt/sftp-fastdl:main`

## Example Docker Compose file

```yaml
version: "3.3"

networks:
  caddy:
    external: true

services:
  sftp-fastdl:
    image: ghcr.io/rob0rt/sftp-fastdl:main
    container_name: fastdl
    environment:
      PORT: 3000
      SFTP_HOST: 192.168.1.92
      SFTP_PORT: 2224
      SFTP_USERNAME: user
      SFTP_PASSWORD: password
      SFTP_PATH: /jbep3
```