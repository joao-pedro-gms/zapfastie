---
title: WhatsApp para Linux
description: O WhatsApp não tem app para Linux. O ZapFastie é um cliente nativo de WhatsApp para Linux, macOS e Windows, em Rust, sem navegador por dentro. Abre em menos de um segundo.
permalink: /pt-br/
lang: pt-BR
seo:
  title: "WhatsApp para Linux: um app nativo"
alternates:
  - lang: en
    url: /whatsapp-on-linux/
  - lang: pt-BR
    url: /pt-br/
---

*Esta página também está em [inglês](/whatsapp-on-linux/).*

O WhatsApp tem apps para celular, Windows e macOS, e nenhum para Linux. No
Linux, sobram três caminhos:

- **O WhatsApp Web numa aba do navegador.** Funciona em qualquer lugar, mas é
  uma aba: fecha junto com o navegador, e a página com os processos do
  navegador ocupa cerca de um gigabyte de memória.
- **Um app que embrulha o WhatsApp Web.** Dá ao WhatsApp uma janela e um
  ícone na bandeja, mas por dentro continua sendo a página web com um motor de
  navegador.
- **Um cliente nativo como o ZapFastie.** Ele fala diretamente o protocolo de
  dispositivos conectados do WhatsApp, sem página web e sem navegador.

O ZapFastie é um único programa escrito em Rust. No nosso teste no Linux, ele
abre em menos de um segundo e usa cerca de 200 MB de memória, contra 1,13 GB
do WhatsApp Web com os processos do Chromium.
[Veja as medições](/benchmarks/) (em inglês).

<img class="VPImage dark" src="{{ '/screenshot.png' | relative_url }}" alt="O ZapFastie mostrando uma conversa com um anexo, mensagens de voz, reações, uma resposta citada e uma prévia de link" width="1280" height="960">
<img class="VPImage light" src="{{ '/screenshot-light.png' | relative_url }}" alt="O ZapFastie mostrando uma conversa com um anexo, mensagens de voz, reações, uma resposta citada e uma prévia de link" width="1280" height="960">

A maior parte do app já está em português do Brasil, e ele segue o idioma do
seu sistema.

## Como instalar

O ZapFastie ainda não tem pacote publicado: o projeto ainda não publicou uma
versão, então não existe `.deb`, `.rpm`, Flatpak, AppImage nem instalador para
baixar. O que funciona hoje é compilar, o que leva alguns minutos depois que as
dependências de build estão no lugar.

| Distribuição | Como |
| --- | --- |
| Qualquer distribuição Linux | Clone o repositório, rode `cargo install --path .` e inicie o `zapfastie`. [Getting started](/getting-started/) lista os pacotes que o build precisa. |
| Arch, Manjaro, CachyOS, Omarchy | O mesmo build. A pasta `packaging/arch/zapfastie/` guarda o modelo de pacote que o fluxo de release transforma em pacote do AUR, e esse pacote ainda não foi publicado. |
| NixOS e Nix | `nix run .#zapfastie` dentro do repositório clonado, ou o pacote do flake. O flake tem um módulo do Home Manager. |
| Entrada no lançador | Depois de `cargo build --release --locked`, rode `packaging/install-user.sh` para instalar o binário, o arquivo `.desktop` e o ícone em `~/.local`. |

Para macOS e Windows existem hoje só os scripts de empacotamento em
`packaging/`: nenhum binário está publicado.

Depois, abra o ZapFastie e conecte pelo celular, em **Dispositivos
conectados**, com o QR code ou com um código de oito caracteres digitado no
celular.

## O que ele faz

- **Suas conversas guardadas no computador.** As mensagens ficam num arquivo
  criptografado, com a chave no chaveiro do sistema, e as mais antigas vêm do
  celular conforme você rola a conversa.
- **Com a cara do seu desktop.** Tema claro, escuro ou o do sistema, temas de
  cores próprios e, no Omarchy, o tema do seu desktop.
- **Continua aberto.** Fechar a janela deixa o ZapFastie na bandeja, e as
  notificações abrem a conversa na mensagem que anunciaram.
- **Vários números.** Conecte mais de um número de WhatsApp e alterne entre
  eles pela sua foto no pé do rail ao lado da lista de conversas.
- **Chamadas de voz e de vídeo.** Faça e atenda chamadas individuais,
  escolhendo microfone, alto-falante e câmera.

## Perguntas

### Dá para instalar o WhatsApp no Linux?

O app oficial do WhatsApp, não: ele não existe. Dá para usar o WhatsApp Web
no navegador ou instalar um cliente como o ZapFastie, que se conecta à sua conta
do mesmo jeito que o WhatsApp Web.

### Como usar o WhatsApp no Ubuntu?

Compile o ZapFastie a partir do código, como o [Getting started](/getting-started/)
descreve (em inglês), e abra o programa. Depois escaneie o QR code com o
celular em **Dispositivos conectados**.

### O celular precisa ficar ligado?

Não. O ZapFastie é um dispositivo conectado, como o WhatsApp Web, então envia e
recebe mensagens com o celular desligado. O histórico mais antigo vem do
celular, então buscar mensagens além das que o ZapFastie já guardou espera o
celular ficar online.

### É gratuito?

Sim. O ZapFastie é código aberto, sob a licença MIT, e o código está
[no GitHub](https://github.com/joao-pedro-gms/zapfastie).

### É seguro para a minha conta?

O ZapFastie é um cliente não oficial, e usar um pode ir contra os termos do
WhatsApp. Ele se conecta como um dispositivo comum e nunca automatiza nem
envia mensagens em massa, mas o WhatsApp ainda pode restringir contas que
usam clientes não oficiais.

O ZapFastie é um projeto independente, sem vínculo com o WhatsApp LLC ou a Meta.
WhatsApp é uma marca registrada do WhatsApp LLC.
