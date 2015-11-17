(ns verwalter.core
  (:require
    [rum.core :as rum]
    [ajax.core :as ajax]
    [verwalter.config :as config]))

(enable-console-print!)

(rum/defc label [n text]
  [:.label (repeat n text)])


(def cfg (atom {}))
(def state (atom {}))

(rum/defc page []
  (config/render @cfg (rum/cursor state [:config])))

(let [comp (rum/mount (page) (.getElementById js/document "app"))]
    (add-watch cfg :page
        (fn [_ _ _ _]
            (rum/request-render comp))))


(defn got_config [new_config]
    (do
        (.log js/console "Config" new_config)
        (reset! cfg new_config)))

(ajax/GET "/v1/config" {
    :handler got_config
    :response-format :json
    })
