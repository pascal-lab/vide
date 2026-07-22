## Website Development
If you want to run the website locally, you can do so by following steps.
1. **Install emscripten**. If you have emscripten installed, you can skip this step.

    The playground needs [emscripten](https://emscripten.org/index.html) to build the wasm module. You can follow the [official guide](https://emscripten.org/docs/getting_started/downloads.html) to install it.

    > Don't forget to activate the emscripten environment by running `source /path/to/emsdk/emsdk_env.sh` in your terminal.

2. **Setup dependencies**.
    This will automatically install all the npm dependencies and build the vide wasm lsp.
   ```bash
   npm run setup
   ```

3. **Run the website**.
   ```bash
   npm run dev
   ```
