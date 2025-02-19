const path = require('path');
const webpack = require('webpack');
const HtmlWebpackPlugin = require('html-webpack-plugin');
const WasmPackPlugin = require('@wasm-tool/wasm-pack-plugin');
const CopyWebpackPlugin = require('copy-webpack-plugin');

module.exports = {
  entry: ['core-js/stable', './www/index.js'],
  output: {
    path: path.resolve(__dirname, 'dist'),
    filename: 'bundle.js',
    publicPath: './',
  },
  experiments: {
    asyncWebAssembly: true,
    syncWebAssembly: true,
  },
  mode: 'production',
  plugins: [
    new HtmlWebpackPlugin({
      template: 'www/index.html',
      filename: 'index.html',
    }),
    new WasmPackPlugin({
      crateDirectory: path.resolve(__dirname, '.'),
      outDir: 'pkg',
      forceMode: 'production',
    }),
    new CopyWebpackPlugin({
      patterns: [
        { from: 'www/delta_composer.js', to: '.' },
        { from: 'static', to: 'static' }
      ],
    }),
    new webpack.ProvidePlugin({
      Buffer: ['buffer', 'Buffer'],
      process: 'process/browser',
    }),
  ],
  module: {
    rules: [
      {
        test: /\.css$/,
        use: ['style-loader', 'css-loader'],
      },
      {
        test: /\.js$/,
        exclude: /node_modules/,
        use: {
          loader: 'babel-loader',
          options: {
            presets: [
              ['@babel/preset-env', {
                useBuiltIns: 'entry',
                corejs: 3,
                targets: {
                  browsers: [
                    'last 2 Chrome versions',
                    'last 2 Firefox versions',
                    'last 2 Safari versions',
                    'last 2 Edge versions'
                  ]
                }
              }]
            ],
            plugins: ['@babel/plugin-proposal-class-properties']
          },
        },
      },
    ],
  },
  resolve: {
    extensions: ['.js', '.wasm'],
    fallback: {
      "http": require.resolve("stream-http"),
      "https": require.resolve("https-browserify"),
      "os": require.resolve("os-browserify/browser"),
      "querystring": require.resolve("querystring-es3"),
      "zlib": require.resolve("browserify-zlib"),
      "stream": require.resolve("stream-browserify"),
      "vm": require.resolve("vm-browserify"),
      "assert": require.resolve("assert/"),
      "buffer": require.resolve("buffer/"),
      "url": require.resolve("url/"),
      "util": require.resolve("util/"),
      "crypto": require.resolve("crypto-browserify"),
      "path": require.resolve("path-browserify"),
      "process": require.resolve("process/browser"),
      "net": false,
      "tls": false,
      "fs": false
    }
  },
  ignoreWarnings: [/Failed to parse source map/],
  optimization: {
    moduleIds: 'deterministic',
    minimize: true
  }
};
