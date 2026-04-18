import axios from 'axios'

const client = axios.create({
  baseURL: '/api/v1',
  withCredentials: true, // send HttpOnly session cookie
})

export default client
