var webpack = require('webpack')
var DEV = process.env['NODE_ENV'] != 'production';
module.exports = {
    context: __dirname,
    entry: DEV ? [
        "./index",
        "webpack-dev-server/client?http://localhost:8080",
        "webpack/hot/only-dev-server",
    ] : "./index",
    output: {
        path: __dirname + "/../public/js",
        filename: "bundle.js",
        publicPath: '/js/',
    },
    module: {
        loaders: [{
            test: /\.khufu$/,
            loaders: ['babel', 'khufu'],
            exclude: /node_modules/,
        }, {
            test: /\.js$/,
            loaders: ['babel'],
            exclude: /node_modules/,
        }],
    },
    babel: {
        "plugins": [
            "transform-strict-mode",
            "transform-object-rest-spread",
            "transform-es2015-block-scoping",
        ],
    },
    resolve: {
        modules: ["/usr/local/lib/node_modules", "/usr/lib/node_modules"],
    },
    resolveLoader: {
        mainFields: ["webpackLoader", "main", "browser"],
        modules: [
            "/work/node_modules",
            "/usr/local/lib/node_modules",
            "/usr/lib/node_modules"],
    },
    devServer: {
        contentBase: '../public',
        proxy: {
            '/*.json': {
                target: 'http://localhost:8379',
                secure: false,
            },
        },
        publicPath: '/js/',
        hot: true,
        historyApiFallback: true,
    },
    khufu: {
        static_attrs: !DEV,
    },
    plugins: [
        new webpack.NoErrorsPlugin(),
        new webpack.DefinePlugin({
            VERSION: JSON.stringify(process.env['CANTAL_VERSION']),
            "process.env.NODE_ENV": JSON.stringify(process.env['NODE_ENV']),
            DEBUG: DEV,
        }),
    ],
}

