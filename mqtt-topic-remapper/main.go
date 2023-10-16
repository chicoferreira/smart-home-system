package main

import (
	"flag"
	"fmt"
	"github.com/BurntSushi/toml"
	mqtt "github.com/eclipse/paho.mqtt.golang"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"
)

type Config struct {
	Remaps []Remap `toml:"remap"`
}

func loadTomlFromFile(file string) (Config, error) {
	fmt.Println("Loading config from file", file)
	var config Config
	var _, err = toml.DecodeFile(file, &config)
	return config, err
}

type Remap struct {
	From          string            `toml:"from"`
	To            string            `toml:"to"`
	ValueMappings map[string]string `toml:"message"`
}

func (r Remap) remap(payload string) string {
	for from, to := range r.ValueMappings {
		payload = strings.ReplaceAll(payload, from, to)
	}
	return payload
}

func createClientOptions(brokerUri string) *mqtt.ClientOptions {
	opts := mqtt.NewClientOptions()
	opts.AddBroker(fmt.Sprintf("tcp://%s", brokerUri))
	opts.SetClientID("mqtt-topic-remapper")
	opts.SetUsername(os.Getenv("MQTT_USERNAME"))
	opts.SetPassword(os.Getenv("MQTT_PASSWORD"))
	return opts
}

func connect(opts *mqtt.ClientOptions) mqtt.Client {
	client := mqtt.NewClient(opts)
	token := client.Connect()

	retries := 3
	for !token.WaitTimeout(3 * time.Second) {
		if retries == 0 {
			panic("failed to connect to MQTT server")
		}
		fmt.Println("Retrying connection to MQTT server...")
		retries--
	}

	if err := token.Error(); err != nil {
		panic(fmt.Errorf("failed to connect to MQTT server: %s", err))
	}

	return client
}

func main() {
	var configPath string
	flag.StringVar(&configPath, "config", "config.toml", "Path to config file")
	flag.Parse()

	fmt.Println("Starting mqtt-topic-remapper...")

	config, err := loadTomlFromFile(configPath)
	if err != nil {
		fmt.Println("Error loading config file:", err)
		return
	}

	fmt.Println("Loaded config:", config)

	keepAlive := make(chan os.Signal)
	signal.Notify(keepAlive, os.Interrupt, syscall.SIGTERM)

	opts := createClientOptions(os.Getenv("MQTT_SERVER_URI"))
	client := connect(opts)

	var remapMap = make(map[string]Remap)

	for _, remap := range config.Remaps {
		remapMap[remap.From] = remap
		fmt.Printf("Subscribing remap from %s to %s (value mappings: %s)...\n", remap.From, remap.To, remap.ValueMappings)
		client.Subscribe(remap.From, 0, nil).Wait()
	}

	client.AddRoute("#", func(client mqtt.Client, msg mqtt.Message) {
		message := string(msg.Payload())
		remap, ok := remapMap[msg.Topic()]
		if !ok {
			fmt.Printf("Impossible state: No remap found for topic %s\n", msg.Topic())
			return
		}

		remappedMessage := remap.remap(string(msg.Payload()))

		fmt.Printf("Converting message %s: '%s' -> %s: '%s'\n", msg.Topic(), message, remap.To, remappedMessage)
		go client.Publish(remap.To, 0, false, remappedMessage)
	})

	<-keepAlive
	fmt.Println("Shutting down mqtt-topic-remapper...")
	client.Disconnect(250)
}
