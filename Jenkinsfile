pipeline {
    agent any

    options {
        timestamps()
        disableConcurrentBuilds()
        buildDiscarder(logRotator(numToKeepStr: '20'))
        skipDefaultCheckout(true)
        timeout(time: 30, unit: 'MINUTES')
    }

    environment {
        COMPOSE_FILE = 'docker-compose.yml'
        COMPOSE_PROJECT_NAME = 'teras-telegram-bot'
        CONTAINER_NAME = 'teras'
    }

    stages {
        stage('Checkout') {
            steps {
                deleteDir()
                checkout scm
            }
        }

        stage('Prepare Environment') {
            steps {
                script {
                    echo "-------------------------------------------------------"
                    echo "STAGE: Setup Environment Variables from Infisical Plugin"
                    echo "-------------------------------------------------------"
                }
                withInfisical(
                    configuration: [
                        infisicalCredentialId: 'infisical-satuarah',
                        infisicalEnvironmentSlug: 'prod',
                        infisicalProjectSlug: 'satuarah-vs-o4',
                        infisicalUrl: 'https://app.infisical.com'
                    ],
                    infisicalSecrets: [
                        infisicalSecret(
                            includeImports: true,
                            path: '/teras',
                            secretValues: [
                                [infisicalKey: 'PORT', isRequired: false],
                                [infisicalKey: 'TELEGRAM_BOT_TOKEN', isRequired: true],
                                [infisicalKey: 'TELEGRAM_API_URL', isRequired: false],
                                [infisicalKey: 'DOMAIN', isRequired: false],
                                [infisicalKey: 'WEBHOOK_FORWARD_URL', isRequired: false],
                                [infisicalKey: 'GEMINI_API_KEY', isRequired: false],
                                [infisicalKey: 'PYTHON_BACKEND_URL', isRequired: false],
                                [infisicalKey: 'TIKTOK_API_KEY', isRequired: false]
                            ]
                        )
                    ]
                ) {
                    sh '''
                        set -eu

                        cat <<EOF > .env
PORT=${PORT:-3001}
TELEGRAM_BOT_TOKEN=${TELEGRAM_BOT_TOKEN}
TELEGRAM_API_URL=${TELEGRAM_API_URL:-http://telegram-bot-api:8081}
DOMAIN=${DOMAIN:-}
WEBHOOK_FORWARD_URL=${WEBHOOK_FORWARD_URL:-}
GEMINI_API_KEY=${GEMINI_API_KEY:-}
PYTHON_BACKEND_URL=${PYTHON_BACKEND_URL:-http://bantu-backend-python:25334}
TIKTOK_API_KEY=${TIKTOK_API_KEY:-}
EOF
                        cp .env .env.production
                        chmod 600 .env .env.production
                        echo "Environment file (.env & .env.production) generated successfully."
                    '''
                }
            }
        }

        stage('Build & Deploy') {
            steps {
                sh '''
                    set -eu
                    docker compose -p ${COMPOSE_PROJECT_NAME} -f ${COMPOSE_FILE} build
                    docker rm -f ${CONTAINER_NAME} || true
                    docker compose -p ${COMPOSE_PROJECT_NAME} -f ${COMPOSE_FILE} up -d --force-recreate --remove-orphans
                '''
            }
        }

        stage('Health Check') {
            steps {
                sh '''
                    set -eu
                    sleep 5
                    if ! docker ps --format '{{.Names}}' | grep -q "^${CONTAINER_NAME}\\$"; then
                        echo "Container ${CONTAINER_NAME} is not running!"
                        docker logs ${CONTAINER_NAME} || true
                        exit 1
                    fi
                '''
            }
        }
    }
}
